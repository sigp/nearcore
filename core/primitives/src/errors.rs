use crate::types::{AccountId, Balance, Nonce};
use near_crypto::PublicKey;
use std::fmt::Display;

/// Internal
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Key-value db internal failure
    StorageInternalError,
    /// Storage is PartialStorage and requested a missing trie node
    TrieNodeMissing,
    /// Either invalid state or key-value db is corrupted.
    /// For PartialStorage it cannot be corrupted.
    /// Error message is unreliable and for debugging purposes only. It's also probably ok to
    /// panic in every place that produces this error.
    /// We can check if db is corrupted by verifying everything in the state trie.
    StorageInconsistentState(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.write_str(&format!("{:?}", self))
    }
}

impl std::error::Error for StorageError {}

/// Internal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasOverflowError;
/// Internal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceOverflowError;

/// External
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidTxError {
    InvalidSigner(AccountId),
    SignerDoesNotExist(AccountId),
    InvalidAccessKey(InvalidAccessKeyError),
    InvalidNonce(Nonce, Nonce),
    InvalidReceiver(AccountId),
    InvalidSignature,
    NotEnoughBalance(AccountId, Balance, Balance),
    RentUnpaid(AccountId),
    CostOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidAccessKeyError {
    AccessKeyNotFound(AccountId, PublicKey),
    ReceiverMismatch(AccountId, AccountId),
    MethodNameMismatch(String),
    ActionError,
    NotEnoughAllowance(AccountId, PublicKey, Balance, Balance),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionCallError {
    CompilationError(CompilationError),
    InvalidMethodError(InvalidMethodError),
    RuntimeError(RuntimeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidMethodError {
    MethodEmptyName,
    MethodUTF8Error,
    MethodNotFound,
    MethodInvalidSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    BadUTF16,
    BadUTF8,
    GasExceeded,
    GasLimitExceeded,
    BalanceExceeded,
    EmptyMethodName,

    InvalidReceiptIndex,
    InvalidAccountId,
    InvalidMethodName,

    GuestPanic,
    IntegerOverflow,
    InvalidIteratorIndex,
    InvalidPromiseIndex,
    CannotReturnJointPromise,
    InvalidPromiseResultIndex,
    InvalidRegisterId,
    IteratorWasInvalidated,
    MemoryAccessViolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareError {
    Serialization,
    Deserialization,
    InternalMemoryDeclared,
    GasInstrumentation,
    StackHeightInstrumentation,
    Instantiate,
    Memory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationError {
    CodeDoesNotExist,
    PrepareError(PrepareError),
    WasmerCompileError,
}

impl Display for InvalidTxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            InvalidTxError::InvalidSigner(signer_id) => {
                write!(f, "Invalid signer account ID {:?} according to requirements", signer_id)
            }
            InvalidTxError::SignerDoesNotExist(signer_id) => {
                write!(f, "Signer {:?} does not exist", signer_id)
            }
            InvalidTxError::InvalidAccessKey(access_key_error) => access_key_error.fmt(f),
            InvalidTxError::InvalidNonce(tx_nonce, ak_nonce) => write!(
                f,
                "Transaction nonce {} must be larger than nonce of the used access key {}",
                tx_nonce, ak_nonce
            ),
            InvalidTxError::InvalidReceiver(receiver_id) => {
                write!(f, "Invalid receiver account ID {:?} according to requirements", receiver_id)
            }
            InvalidTxError::InvalidSignature => {
                write!(f, "Transaction is not signed with a public key of the signer")
            }
            InvalidTxError::NotEnoughBalance(signer_id, balance, cost) => write!(
                f,
                "Sender {} does not have enough balance {} for operation costing {}",
                signer_id, balance, cost
            ),
            InvalidTxError::RentUnpaid(signer_id) => {
                write!(f, "Failed to execute, because the account {} wouldn't have enough to pay required rent", signer_id)
            }
            InvalidTxError::CostOverflow => {
                write!(f, "Transaction gas or balance cost is too high") 
            }
        }
    }
}

impl From<InvalidAccessKeyError> for InvalidTxError {
    fn from(error: InvalidAccessKeyError) -> Self {
        InvalidTxError::InvalidAccessKey(error)
    }
}

impl From<GasOverflowError> for InvalidTxError {
    fn from(_: GasOverflowError) -> Self {
        InvalidTxError::CostOverflow
    }
}

impl From<BalanceOverflowError> for InvalidTxError {
    fn from(_: BalanceOverflowError) -> Self {
        InvalidTxError::CostOverflow
    }
}

impl Display for InvalidAccessKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            InvalidAccessKeyError::AccessKeyNotFound(account_id, public_key) => write!(
                f,
                "Signer {:?} doesn't have access key with the given public_key {}",
                account_id, public_key
            ),
            InvalidAccessKeyError::ReceiverMismatch(tx_receiver, ak_receiver) => write!(
                f,
                "Transaction receiver_id {:?} doesn't match the access key receiver_id {:?}",
                tx_receiver, ak_receiver
            ),
            InvalidAccessKeyError::MethodNameMismatch(method_name) => write!(
                f,
                "Transaction method name {:?} isn't allowed by the access key",
                method_name
            ),
            InvalidAccessKeyError::ActionError => {
                write!(f, "The used access key requires exactly one FunctionCall action")
            }
            InvalidAccessKeyError::NotEnoughAllowance(account_id, public_key, allowance, cost) => {
                write!(
                    f,
                    "Access Key {}:{} does not have enough balance {} for transaction costing {}",
                    account_id, public_key, allowance, cost
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidTxErrorOrStorageError {
    InvalidTxError(InvalidTxError),
    StorageError(StorageError),
}

impl From<StorageError> for InvalidTxErrorOrStorageError {
    fn from(e: StorageError) -> Self {
        InvalidTxErrorOrStorageError::StorageError(e)
    }
}

impl<T> From<T> for InvalidTxErrorOrStorageError
where
    T: Into<InvalidTxError>,
{
    fn from(e: T) -> Self {
        InvalidTxErrorOrStorageError::InvalidTxError(e.into())
    }
}
