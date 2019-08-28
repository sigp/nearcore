use std::convert::From;
use std::convert::{Into, TryFrom, TryInto};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Duration;

use actix::dev::{MessageResponse, ResponseChannel};
use actix::{Actor, Addr, Message};
use chrono::{DateTime, Utc};
use protobuf::well_known_types::UInt32Value;
use protobuf::{RepeatedField, SingularPtrField};
use serde_derive::{Deserialize, Serialize};
use tokio::net::TcpStream;

use near_chain::{Block, BlockApproval, BlockHeader, Weight};
use near_primitives::crypto::signature::{sign, PublicKey, SecretKey, Signature};
use near_primitives::hash::{hash, CryptoHash};
use near_primitives::logging::pretty_str;
use near_primitives::serialize::{BaseEncode, Decode};
use near_primitives::transaction::{ReceiptTransaction, SignedTransaction};
use near_primitives::types::{AccountId, BlockIndex, ShardId};
use near_primitives::utils::{proto_to_type, to_string_value};
use near_protos::network as network_proto;

use crate::peer::Peer;
use std::collections::HashMap;

/// Current latest version of the protocol
pub const PROTOCOL_VERSION: u32 = 2;

/// Peer id is the public key.
#[derive(Copy, Clone, Eq, PartialOrd, Ord, PartialEq, Serialize, Deserialize)]
pub struct PeerId(PublicKey);

impl PeerId {
    pub fn public_key(&self) -> PublicKey {
        self.0
    }
}

impl From<PeerId> for Vec<u8> {
    fn from(peer_id: PeerId) -> Vec<u8> {
        (&peer_id.0).into()
    }
}

impl From<PublicKey> for PeerId {
    fn from(public_key: PublicKey) -> PeerId {
        PeerId(public_key)
    }
}

impl TryFrom<Vec<u8>> for PeerId {
    type Error = Box<dyn std::error::Error>;

    fn try_from(bytes: Vec<u8>) -> Result<PeerId, Self::Error> {
        Ok(PeerId(bytes.try_into()?))
    }
}

impl std::convert::AsRef<[u8]> for PeerId {
    fn as_ref(&self) -> &[u8] {
        &(self.0).0[..]
    }
}

impl Hash for PeerId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.0.as_ref());
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", pretty_str(&self.0.to_base(), 4))
    }
}

/// Peer information.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: PeerId,
    pub addr: Option<SocketAddr>,
    pub account_id: Option<AccountId>,
}

impl PeerInfo {
    pub fn addr_port(&self) -> Option<u16> {
        self.addr.map(|addr| addr.port())
    }
}

impl PeerInfo {
    pub fn new(id: PeerId, addr: SocketAddr) -> Self {
        PeerInfo { id, addr: Some(addr), account_id: None }
    }
}

impl fmt::Display for PeerInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(acc) = self.account_id.as_ref() {
            write!(f, "({}, {:?}, {})", self.id, self.addr, acc)
        } else {
            write!(f, "({}, {:?})", self.id, self.addr)
        }
    }
}

impl TryFrom<&str> for PeerInfo {
    type Error = Box<dyn std::error::Error>;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let chunks: Vec<_> = s.split("@").collect();
        if chunks.len() != 2 {
            return Err(format!("Invalid peer info format, got {}, must be id@ip_addr", s).into());
        }
        Ok(PeerInfo {
            id: PublicKey::try_from(chunks[0])?.into(),
            addr: Some(
                chunks[1].parse().map_err(|err| {
                    format!("Invalid ip address format for {}: {}", chunks[1], err)
                })?,
            ),
            account_id: None,
        })
    }
}

impl TryFrom<network_proto::PeerInfo> for PeerInfo {
    type Error = Box<dyn std::error::Error>;

    fn try_from(proto: network_proto::PeerInfo) -> Result<Self, Self::Error> {
        let addr = proto.addr.into_option().and_then(|s| s.value.parse::<SocketAddr>().ok());
        let account_id = proto.account_id.into_option().map(|s| s.value);
        Ok(PeerInfo { id: PublicKey::try_from(proto.id)?.into(), addr, account_id })
    }
}

impl From<PeerInfo> for network_proto::PeerInfo {
    fn from(peer_info: PeerInfo) -> network_proto::PeerInfo {
        let id = peer_info.id;
        let addr = SingularPtrField::from_option(
            peer_info.addr.map(|s| to_string_value(format!("{}", s))),
        );
        let account_id = SingularPtrField::from_option(peer_info.account_id.map(to_string_value));
        network_proto::PeerInfo { id: (&id.0).into(), addr, account_id, ..Default::default() }
    }
}

/// Peer chain information.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct PeerChainInfo {
    /// Genesis hash.
    pub genesis: CryptoHash,
    /// Last known chain height of the peer.
    pub height: BlockIndex,
    /// Last known chain weight of the peer.
    pub total_weight: Weight,
}

impl TryFrom<network_proto::PeerChainInfo> for PeerChainInfo {
    type Error = Box<dyn std::error::Error>;

    fn try_from(proto: network_proto::PeerChainInfo) -> Result<Self, Self::Error> {
        Ok(PeerChainInfo {
            genesis: proto.genesis.try_into()?,
            height: proto.height,
            total_weight: proto.total_weight.into(),
        })
    }
}

impl From<PeerChainInfo> for network_proto::PeerChainInfo {
    fn from(chain_peer_info: PeerChainInfo) -> network_proto::PeerChainInfo {
        network_proto::PeerChainInfo {
            genesis: chain_peer_info.genesis.into(),
            height: chain_peer_info.height,
            total_weight: chain_peer_info.total_weight.to_num(),
            ..Default::default()
        }
    }
}

/// Peer type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PeerType {
    /// Inbound session
    Inbound,
    /// Outbound session
    Outbound,
}

/// Peer status.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PeerStatus {
    /// Waiting for handshake.
    Connecting,
    /// Ready to go.
    Ready,
    /// Banned, should shutdown this peer.
    Banned(ReasonForBan),
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Handshake {
    /// Protocol version.
    pub version: u32,
    /// Sender's peer id.
    pub peer_id: PeerId,
    /// Sender's listening addr.
    pub listen_port: Option<u16>,
    /// Peer's chain information.
    pub chain_info: PeerChainInfo,
}

impl Handshake {
    pub fn new(peer_id: PeerId, listen_port: Option<u16>, chain_info: PeerChainInfo) -> Self {
        Handshake { version: PROTOCOL_VERSION, peer_id, listen_port, chain_info }
    }
}

impl TryFrom<network_proto::Handshake> for Handshake {
    type Error = Box<dyn std::error::Error>;

    fn try_from(proto: network_proto::Handshake) -> Result<Self, Self::Error> {
        let listen_port = proto.listen_port.into_option().map(|v| v.value as u16);
        let peer_id: PublicKey = proto.peer_id.try_into().map_err(|e| format!("{}", e))?;
        let chain_info = proto_to_type(proto.chain_info)?;
        Ok(Handshake { version: proto.version, peer_id: peer_id.into(), listen_port, chain_info })
    }
}

impl From<Handshake> for network_proto::Handshake {
    fn from(handshake: Handshake) -> network_proto::Handshake {
        let listen_port = SingularPtrField::from_option(handshake.listen_port.map(|v| {
            let mut res = UInt32Value::new();
            res.set_value(u32::from(v));
            res
        }));
        network_proto::Handshake {
            version: handshake.version,
            peer_id: handshake.peer_id.into(),
            listen_port,
            chain_info: SingularPtrField::some(handshake.chain_info.into()),
            ..Default::default()
        }
    }
}

/// Account route description
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct AnnounceAccountRoute {
    pub peer_id: PeerId,
    pub hash: CryptoHash,
    pub signature: Signature,
}

impl TryFrom<network_proto::AnnounceAccountRoute> for AnnounceAccountRoute {
    type Error = Box<dyn std::error::Error>;

    fn try_from(proto: network_proto::AnnounceAccountRoute) -> Result<Self, Self::Error> {
        let peer_id: PeerId = proto.peer_id.try_into().map_err(|e| format!("{}", e))?;
        let hash: CryptoHash = proto.hash.try_into().map_err(|e| format!("{}", e))?;
        let signature: Signature = proto.signature.try_into().map_err(|e| format!("{}", e))?;
        Ok(AnnounceAccountRoute { peer_id, hash, signature })
    }
}

impl From<AnnounceAccountRoute> for network_proto::AnnounceAccountRoute {
    fn from(announce_account_route: AnnounceAccountRoute) -> network_proto::AnnounceAccountRoute {
        network_proto::AnnounceAccountRoute {
            peer_id: announce_account_route.peer_id.into(),
            hash: announce_account_route.hash.into(),
            signature: announce_account_route.signature.into(),
            ..Default::default()
        }
    }
}

/// Account announcement information
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct AnnounceAccount {
    /// AccountId to be announced
    pub account_id: AccountId,
    /// This announcement is only valid for this `epoch`
    pub epoch: CryptoHash,
    /// Complete route description to account id
    /// First element of the route (header) contains:
    ///     peer_id owner of the account_id
    ///     hash of the announcement
    ///     signature with account id secret key
    /// Subsequent elements of the route contain:
    ///     peer_id of intermediates hop in the route
    ///     hash built using previous hash and peer_id
    ///     signature with peer id secret key
    pub route: Vec<AnnounceAccountRoute>,
}

impl AnnounceAccount {
    pub fn new(
        account_id: AccountId,
        epoch_hash: CryptoHash,
        peer_id: PeerId,
        hash: CryptoHash,
        signature: Signature,
    ) -> Self {
        let route = vec![AnnounceAccountRoute { peer_id, hash, signature }];
        Self { account_id, epoch: epoch_hash, route }
    }

    pub fn build_header_hash(
        account_id: &AccountId,
        peer_id: &PeerId,
        epoch: CryptoHash,
    ) -> CryptoHash {
        hash([account_id.as_bytes(), peer_id.as_ref(), epoch.as_ref()].concat().as_slice())
    }

    pub fn header_hash(&self) -> CryptoHash {
        AnnounceAccount::build_header_hash(
            &self.account_id,
            &self.route.first().unwrap().peer_id,
            self.epoch,
        )
    }

    pub fn header(&self) -> &AnnounceAccountRoute {
        self.route.first().unwrap()
    }

    pub fn peer_id_sender(&self) -> PeerId {
        self.route.last().unwrap().peer_id
    }

    pub fn num_hops(&self) -> usize {
        self.route.len() - 1
    }

    pub fn extend(&mut self, peer_id: PeerId, secret_key: &SecretKey) {
        let last_hash = self.route.last().unwrap().hash;
        let new_hash = hash([last_hash.as_ref(), peer_id.as_ref()].concat().as_slice());
        let signature = sign(new_hash.as_ref(), secret_key);
        self.route.push(AnnounceAccountRoute { peer_id, hash: new_hash, signature })
    }
}

impl TryFrom<network_proto::AnnounceAccount> for AnnounceAccount {
    type Error = Box<dyn std::error::Error>;

    fn try_from(proto: network_proto::AnnounceAccount) -> Result<Self, Self::Error> {
        let epoch: CryptoHash = proto.epoch.try_into().map_err(|e| format!("{}", e))?;
        Ok(AnnounceAccount {
            account_id: proto.account_id,
            epoch,
            route: proto
                .route
                .into_iter()
                .filter_map(|hop| match hop.try_into() {
                    Ok(hop) => Some(hop),
                    Err(_) => None,
                })
                .collect(),
        })
    }
}

impl From<AnnounceAccount> for network_proto::AnnounceAccount {
    fn from(announce_account: AnnounceAccount) -> network_proto::AnnounceAccount {
        network_proto::AnnounceAccount {
            account_id: announce_account.account_id,
            epoch: announce_account.epoch.into(),
            route: RepeatedField::from_iter(
                announce_account.route.into_iter().map(|hop| hop.into()),
            ),
            ..Default::default()
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum PeerMessage {
    Handshake(Handshake),

    PeersRequest,
    PeersResponse(Vec<PeerInfo>),

    BlockHeadersRequest(Vec<CryptoHash>),
    BlockHeaders(Vec<BlockHeader>),
    BlockHeaderAnnounce(BlockHeader),

    BlockRequest(CryptoHash),
    Block(Block),
    BlockApproval(AccountId, CryptoHash, Signature),

    Transaction(SignedTransaction),

    StateRequest(ShardId, CryptoHash),
    StateResponse(ShardId, CryptoHash, Vec<u8>, Vec<ReceiptTransaction>),

    AnnounceAccount(AnnounceAccount),
}

impl fmt::Display for PeerMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerMessage::Handshake(_) => f.write_str("Handshake"),
            PeerMessage::PeersRequest => f.write_str("PeersRequest"),
            PeerMessage::PeersResponse(_) => f.write_str("PeersResponse"),
            PeerMessage::BlockHeadersRequest(_) => f.write_str("BlockHeaderRequest"),
            PeerMessage::BlockHeaders(_) => f.write_str("BlockHeaders"),
            PeerMessage::BlockHeaderAnnounce(_) => f.write_str("BlockHeaderAnnounce"),
            PeerMessage::BlockRequest(_) => f.write_str("BlockRequest"),
            PeerMessage::Block(_) => f.write_str("Block"),
            PeerMessage::BlockApproval(_, _, _) => f.write_str("BlockApproval"),
            PeerMessage::Transaction(_) => f.write_str("Transaction"),
            PeerMessage::StateRequest(_, _) => f.write_str("StateRequest"),
            PeerMessage::StateResponse(_, _, _, _) => f.write_str("StateResponse"),
            PeerMessage::AnnounceAccount(_) => f.write_str("AnnounceAccount"),
        }
    }
}

impl TryFrom<network_proto::PeerMessage> for PeerMessage {
    type Error = Box<dyn std::error::Error>;

    fn try_from(proto: network_proto::PeerMessage) -> Result<Self, Self::Error> {
        match proto.message_type {
            Some(network_proto::PeerMessage_oneof_message_type::hand_shake(hand_shake)) => {
                hand_shake.try_into().map(PeerMessage::Handshake)
            }
            Some(network_proto::PeerMessage_oneof_message_type::peers_request(_)) => {
                Ok(PeerMessage::PeersRequest)
            }
            Some(network_proto::PeerMessage_oneof_message_type::peers_response(peers_response)) => {
                let peers_response = peers_response
                    .peers
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(PeerMessage::PeersResponse(peers_response))
            }
            Some(network_proto::PeerMessage_oneof_message_type::block(block)) => {
                Ok(PeerMessage::Block(block.try_into()?))
            }
            Some(network_proto::PeerMessage_oneof_message_type::block_header_announce(header)) => {
                Ok(PeerMessage::BlockHeaderAnnounce(header.try_into()?))
            }
            Some(network_proto::PeerMessage_oneof_message_type::transaction(transaction)) => {
                Ok(PeerMessage::Transaction(transaction.try_into()?))
            }
            Some(network_proto::PeerMessage_oneof_message_type::block_approval(block_approval)) => {
                Ok(PeerMessage::BlockApproval(
                    block_approval.account_id,
                    block_approval.hash.try_into()?,
                    block_approval.signature.try_into()?,
                ))
            }
            Some(network_proto::PeerMessage_oneof_message_type::block_request(block_request)) => {
                Ok(PeerMessage::BlockRequest(block_request.try_into()?))
            }
            Some(network_proto::PeerMessage_oneof_message_type::block_headers_request(
                block_headers_request,
            )) => Ok(PeerMessage::BlockHeadersRequest(
                block_headers_request
                    .hashes
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Some(network_proto::PeerMessage_oneof_message_type::block_headers(block_headers)) => {
                Ok(PeerMessage::BlockHeaders(
                    block_headers
                        .headers
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            }
            Some(network_proto::PeerMessage_oneof_message_type::state_request(state_request)) => {
                Ok(PeerMessage::StateRequest(
                    state_request.shard_id,
                    state_request.hash.try_into()?,
                ))
            }
            Some(network_proto::PeerMessage_oneof_message_type::state_response(state_response)) => {
                Ok(PeerMessage::StateResponse(
                    state_response.shard_id,
                    state_response.hash.try_into()?,
                    state_response.payload,
                    state_response
                        .receipts
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            }
            Some(network_proto::PeerMessage_oneof_message_type::announce_account(
                announce_account,
            )) => announce_account.try_into().map(PeerMessage::AnnounceAccount),
            None => Err(format!("Unexpected empty message body").into()),
        }
    }
}

impl From<PeerMessage> for network_proto::PeerMessage {
    fn from(message: PeerMessage) -> network_proto::PeerMessage {
        let message_type = match message {
            PeerMessage::Handshake(hand_shake) => {
                Some(network_proto::PeerMessage_oneof_message_type::hand_shake(hand_shake.into()))
            }
            PeerMessage::PeersRequest => {
                Some(network_proto::PeerMessage_oneof_message_type::peers_request(true))
            }
            PeerMessage::PeersResponse(peers_response) => {
                let peers_response = network_proto::PeersResponse {
                    peers: RepeatedField::from_iter(
                        peers_response.into_iter().map(std::convert::Into::into),
                    ),
                    cached_size: Default::default(),
                    unknown_fields: Default::default(),
                };
                Some(network_proto::PeerMessage_oneof_message_type::peers_response(peers_response))
            }
            PeerMessage::Block(block) => {
                Some(network_proto::PeerMessage_oneof_message_type::block(block.into()))
            }
            PeerMessage::BlockHeaderAnnounce(header) => Some(
                network_proto::PeerMessage_oneof_message_type::block_header_announce(header.into()),
            ),
            PeerMessage::Transaction(transaction) => {
                Some(network_proto::PeerMessage_oneof_message_type::transaction(transaction.into()))
            }
            PeerMessage::BlockApproval(account_id, hash, signature) => {
                let block_approval = network_proto::BlockApproval {
                    account_id,
                    hash: hash.into(),
                    signature: signature.into(),
                    cached_size: Default::default(),
                    unknown_fields: Default::default(),
                };
                Some(network_proto::PeerMessage_oneof_message_type::block_approval(block_approval))
            }
            PeerMessage::BlockRequest(hash) => {
                Some(network_proto::PeerMessage_oneof_message_type::block_request(hash.into()))
            }
            PeerMessage::BlockHeadersRequest(hashes) => {
                let request = network_proto::BlockHeaderRequest {
                    hashes: RepeatedField::from_iter(
                        hashes.into_iter().map(std::convert::Into::into),
                    ),
                    cached_size: Default::default(),
                    unknown_fields: Default::default(),
                };
                Some(network_proto::PeerMessage_oneof_message_type::block_headers_request(request))
            }
            PeerMessage::BlockHeaders(headers) => {
                let block_headers = network_proto::BlockHeaders {
                    headers: RepeatedField::from_iter(
                        headers.into_iter().map(std::convert::Into::into),
                    ),
                    cached_size: Default::default(),
                    unknown_fields: Default::default(),
                };
                Some(network_proto::PeerMessage_oneof_message_type::block_headers(block_headers))
            }
            PeerMessage::StateRequest(shard_id, hash) => {
                let state_request = network_proto::StateRequest {
                    shard_id,
                    hash: hash.into(),
                    cached_size: Default::default(),
                    unknown_fields: Default::default(),
                };
                Some(network_proto::PeerMessage_oneof_message_type::state_request(state_request))
            }
            PeerMessage::StateResponse(shard_id, hash, payload, receipts) => {
                let state_response = network_proto::StateResponse {
                    shard_id,
                    hash: hash.into(),
                    payload,
                    receipts: RepeatedField::from_iter(
                        receipts.into_iter().map(std::convert::Into::into),
                    ),
                    cached_size: Default::default(),
                    unknown_fields: Default::default(),
                };
                Some(network_proto::PeerMessage_oneof_message_type::state_response(state_response))
            }
            PeerMessage::AnnounceAccount(announce_account) => {
                Some(network_proto::PeerMessage_oneof_message_type::announce_account(
                    announce_account.into(),
                ))
            }
        };
        network_proto::PeerMessage { message_type, ..Default::default() }
    }
}

/// Configuration for the peer-to-peer manager.
#[derive(Clone)]
pub struct NetworkConfig {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub account_id: Option<AccountId>,
    pub addr: Option<SocketAddr>,
    pub boot_nodes: Vec<PeerInfo>,
    pub handshake_timeout: Duration,
    pub reconnect_delay: Duration,
    pub bootstrap_peers_period: Duration,
    pub peer_max_count: u32,
    /// Duration of the ban for misbehaving peers.
    pub ban_window: Duration,
    /// Remove expired peers.
    pub peer_expiration_duration: Duration,
    /// Maximum number of peer addresses we should ever send.
    pub max_send_peers: u32,
    /// Duration for checking on stats from the peers.
    pub peer_stats_period: Duration,
}

/// Status of the known peers.
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug)]
pub enum KnownPeerStatus {
    Unknown,
    NotConnected,
    Connected,
    Banned(ReasonForBan, DateTime<Utc>),
}

/// Information node stores about known peers.
#[derive(Serialize, Deserialize, Debug)]
pub struct KnownPeerState {
    pub peer_info: PeerInfo,
    pub status: KnownPeerStatus,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

impl KnownPeerState {
    pub fn new(peer_info: PeerInfo) -> Self {
        KnownPeerState {
            peer_info,
            status: KnownPeerStatus::Unknown,
            first_seen: Utc::now(),
            last_seen: Utc::now(),
        }
    }
}

impl TryFrom<Vec<u8>> for KnownPeerState {
    type Error = Box<dyn std::error::Error>;

    fn try_from(bytes: Vec<u8>) -> Result<KnownPeerState, Self::Error> {
        Decode::decode(&bytes).map_err(|err| err.into())
    }
}

/// Actor message that holds the TCP stream from an inbound TCP connection
#[derive(Message)]
pub struct InboundTcpConnect {
    /// Tcp stream of the inbound connections
    pub stream: TcpStream,
}

impl InboundTcpConnect {
    /// Method to create a new InboundTcpConnect message from a TCP stream
    pub fn new(stream: TcpStream) -> InboundTcpConnect {
        InboundTcpConnect { stream }
    }
}

/// Actor message to request the creation of an outbound TCP connection to a peer.
#[derive(Message)]
pub struct OutboundTcpConnect {
    /// Peer information of the outbound connection
    pub peer_info: PeerInfo,
}

#[derive(Message, Clone, Debug)]
pub struct SendMessage {
    pub message: PeerMessage,
}

/// Actor message to consolidate potential new peer.
/// Returns if connection should be kept or dropped.
pub struct Consolidate {
    pub actor: Addr<Peer>,
    pub peer_info: PeerInfo,
    pub peer_type: PeerType,
    pub chain_info: PeerChainInfo,
}

impl Message for Consolidate {
    type Result = bool;
}

/// Unregister message from Peer to PeerManager.
#[derive(Message)]
pub struct Unregister {
    pub peer_id: PeerId,
}

pub struct PeerList {
    pub peers: Vec<PeerInfo>,
}

/// Requesting peers from peer manager to communicate to a peer.
pub struct PeersRequest {}

impl Message for PeersRequest {
    type Result = PeerList;
}

/// Received new peers from another peer.
#[derive(Message)]
pub struct PeersResponse {
    pub peers: Vec<PeerInfo>,
}

impl<A, M> MessageResponse<A, M> for PeerList
where
    A: Actor,
    M: Message<Result = PeerList>,
{
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        if let Some(tx) = tx {
            tx.send(self)
        }
    }
}

/// Ban reason.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize, Deserialize)]
pub enum ReasonForBan {
    None = 0,
    BadBlock = 1,
    BadBlockHeader = 2,
    HeightFraud = 3,
    BadHandshake = 4,
    BadBlockApproval = 5,
    Abusive = 6,
    InvalidSignature = 7,
    InvalidPeerId = 8,
    InvalidHash = 9,
}

#[derive(Message)]
pub struct Ban {
    pub peer_id: PeerId,
    pub ban_reason: ReasonForBan,
}

#[derive(Debug, PartialEq)]
pub enum NetworkRequests {
    /// Fetch information from the network.
    /// Level denote how much information is going to be delivered.
    /// Higher level implies more information. (This is useful for testing)
    FetchInfo { level: usize },
    /// Sends block, either when block was just produced or when requested.
    Block { block: Block },
    /// Sends block header announcement, with possibly attaching approval for this block if
    /// participating in this epoch.
    BlockHeaderAnnounce { header: BlockHeader, approval: Option<BlockApproval> },
    /// Request block with given hash from given peer.
    BlockRequest { hash: CryptoHash, peer_id: PeerId },
    /// Request given block headers.
    BlockHeadersRequest { hashes: Vec<CryptoHash>, peer_id: PeerId },
    /// Request state for given shard at given state root.
    StateRequest { shard_id: ShardId, hash: CryptoHash, peer_id: PeerId },
    /// Ban given peer.
    BanPeer { peer_id: PeerId, ban_reason: ReasonForBan },
    /// Announce account
    AnnounceAccount(AnnounceAccount),
}

/// Combines peer address info and chain information.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FullPeerInfo {
    pub peer_info: PeerInfo,
    pub chain_info: PeerChainInfo,
}

#[derive(Debug)]
pub struct NetworkInfo {
    pub num_active_peers: usize,
    pub peer_max_count: u32,
    pub most_weight_peers: Vec<FullPeerInfo>,
    pub sent_bytes_per_sec: u64,
    pub received_bytes_per_sec: u64,
    // Only send full routes to accounts on demand
    pub routes: Option<HashMap<AccountId, (PeerId, usize)>>,
}

#[derive(Debug)]
pub enum NetworkResponses {
    NoResponse,
    Info(NetworkInfo),
}

impl<A, M> MessageResponse<A, M> for NetworkResponses
where
    A: Actor,
    M: Message<Result = NetworkResponses>,
{
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        if let Some(tx) = tx {
            tx.send(self)
        }
    }
}

impl Message for NetworkRequests {
    type Result = NetworkResponses;
}

#[derive(Debug)]
pub enum NetworkClientMessages {
    /// Received transaction.
    Transaction(SignedTransaction),
    /// Received block header.
    BlockHeader(BlockHeader, PeerId),
    /// Received block, possibly requested.
    Block(Block, PeerId, bool),
    /// Received list of headers for syncing.
    BlockHeaders(Vec<BlockHeader>, PeerId),
    /// Get Chain information from Client.
    GetChainInfo,
    /// Block approval.
    BlockApproval(AccountId, CryptoHash, Signature),
    /// Request headers.
    BlockHeadersRequest(Vec<CryptoHash>),
    /// Request a block.
    BlockRequest(CryptoHash),
    /// State request.
    StateRequest(ShardId, CryptoHash),
    /// State response.
    StateResponse(ShardId, CryptoHash, Vec<u8>, Vec<ReceiptTransaction>),
    /// Account announcement that needs to be validated before being processed
    AnnounceAccount(AnnounceAccount),
}

pub enum NetworkClientResponses {
    /// No response.
    NoResponse,
    /// Valid transaction inserted into mempool as response to Transaction.
    ValidTx,
    /// Invalid transaction inserted into mempool as response to Transaction.
    InvalidTx(String),
    /// Ban peer for malicious behaviour.
    Ban { ban_reason: ReasonForBan },
    /// Chain information.
    ChainInfo { genesis: CryptoHash, height: BlockIndex, total_weight: Weight },
    /// Block response.
    Block(Block),
    /// Headers response.
    BlockHeaders(Vec<BlockHeader>),
    /// Response to state request.
    StateResponse {
        shard_id: ShardId,
        hash: CryptoHash,
        payload: Vec<u8>,
        receipts: Vec<ReceiptTransaction>,
    },
}

impl<A, M> MessageResponse<A, M> for NetworkClientResponses
where
    A: Actor,
    M: Message<Result = NetworkClientResponses>,
{
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        if let Some(tx) = tx {
            tx.send(self)
        }
    }
}

impl Message for NetworkClientMessages {
    type Result = NetworkClientResponses;
}

/// Peer stats query.
pub struct QueryPeerStats {}

/// Peer stats result
#[derive(Debug)]
pub struct PeerStatsResult {
    /// Chain info.
    pub chain_info: PeerChainInfo,
    /// Number of bytes we've received from the peer.
    pub received_bytes_per_sec: u64,
    /// Number of bytes we've sent to the peer.
    pub sent_bytes_per_sec: u64,
    /// Returns if this peer is abusive and should be banned.
    pub is_abusive: bool,
    /// Counts of incoming/outgoing messages from given peer.
    pub message_counts: (u64, u64),
}

impl<A, M> MessageResponse<A, M> for PeerStatsResult
where
    A: Actor,
    M: Message<Result = PeerStatsResult>,
{
    fn handle<R: ResponseChannel<M>>(self, _: &mut A::Context, tx: Option<R>) {
        if let Some(tx) = tx {
            tx.send(self)
        }
    }
}

impl Message for QueryPeerStats {
    type Result = PeerStatsResult;
}
