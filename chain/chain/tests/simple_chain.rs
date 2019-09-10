use near_chain::test_utils::{setup, setup_with_tx_validity_period};
use near_chain::{Block, ErrorKind, Provenance};
use near_crypto::{KeyType, Signature, Signer};
use near_primitives::hash::hash;
use near_primitives::test_utils::init_test_logger;
use near_primitives::transaction::{SignedTransaction, Transaction};
use near_primitives::types::EpochId;
use std::collections::HashMap;

#[test]
fn empty_chain() {
    init_test_logger();
    let (chain, _, _) = setup();
    assert_eq!(chain.head().unwrap().height, 0);
}

#[test]
fn build_chain() {
    init_test_logger();
    let (mut chain, _, signer) = setup();
    for i in 0..4 {
        let prev_hash = chain.head_header().unwrap().hash();
        let prev = chain.get_block(&prev_hash).unwrap();
        let block = Block::empty(&prev, signer.clone());
        let tip =
            chain.process_block(&None, block, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
        assert_eq!(tip.unwrap().height, i + 1);
    }
    assert_eq!(chain.head().unwrap().height, 4);
}

#[test]
fn build_chain_with_orhpans() {
    init_test_logger();
    let (mut chain, _, signer) = setup();
    let mut blocks = vec![chain.get_block(&chain.genesis().hash()).unwrap().clone()];
    for i in 1..4 {
        let block = Block::empty(&blocks[i - 1], signer.clone());
        blocks.push(block);
    }
    let last_block = &blocks[blocks.len() - 1];
    let block = Block::produce(
        &last_block.header,
        10,
        last_block.chunks.clone(),
        last_block.header.inner.epoch_id.clone(),
        vec![],
        HashMap::default(),
        0,
        Some(0),
        signer.clone(),
    );
    assert_eq!(
        chain
            .process_block(&None, block, Provenance::PRODUCED, |_, _, _| {}, |_| {})
            .unwrap_err()
            .kind(),
        ErrorKind::Orphan
    );
    assert_eq!(
        chain
            .process_block(&None, blocks.pop().unwrap(), Provenance::PRODUCED, |_, _, _| {}, |_| {})
            .unwrap_err()
            .kind(),
        ErrorKind::Orphan
    );
    assert_eq!(
        chain
            .process_block(&None, blocks.pop().unwrap(), Provenance::PRODUCED, |_, _, _| {}, |_| {})
            .unwrap_err()
            .kind(),
        ErrorKind::Orphan
    );
    let res = chain.process_block(
        &None,
        blocks.pop().unwrap(),
        Provenance::PRODUCED,
        |_, _, _| {},
        |_| {},
    );
    assert_eq!(res.unwrap().unwrap().height, 10);
    assert_eq!(
        chain
            .process_block(&None, blocks.pop().unwrap(), Provenance::PRODUCED, |_, _, _| {}, |_| {})
            .unwrap_err()
            .kind(),
        ErrorKind::Unfit("already known in store".to_string())
    );
}

#[test]
fn build_chain_with_skips_and_forks() {
    init_test_logger();
    let (mut chain, _, signer) = setup();
    let genesis = chain.get_block(&chain.genesis().hash()).unwrap();
    let b1 = Block::empty(&genesis, signer.clone());
    let b2 = Block::empty_with_height(&genesis, 2, signer.clone());
    let b3 = Block::empty_with_height(&b1, 3, signer.clone());
    let b4 = Block::empty_with_height(&b2, 4, signer.clone());
    let b5 = Block::empty(&b4, signer);
    assert!(chain.process_block(&None, b1, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    assert!(chain.process_block(&None, b2, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    assert!(chain.process_block(&None, b3, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    assert!(chain.process_block(&None, b4, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    assert!(chain.process_block(&None, b5, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    assert!(chain.get_header_by_height(1).is_err());
    assert_eq!(chain.get_header_by_height(5).unwrap().inner.height, 5);
}

#[test]
fn test_apply_expired_tx() {
    init_test_logger();
    let (mut chain, _, signer) = setup_with_tx_validity_period(0);
    let genesis = chain.get_block_by_height(0).unwrap().clone();
    let b1 = Block::empty(&genesis, signer.clone());
    let tx = SignedTransaction::new(
        Signature::empty(KeyType::ED25519),
        Transaction {
            signer_id: "".to_string(),
            public_key: signer.public_key(),
            nonce: 0,
            receiver_id: "".to_string(),
            block_hash: b1.hash(),
            actions: vec![],
        },
    );
    let _b2 = Block::produce(
        &chain.genesis(),
        2,
        genesis.chunks.clone(),
        EpochId::default(),
        vec![tx],
        HashMap::default(),
        0,
        Some(0),
        signer.clone(),
    );
    assert!(chain.process_block(&None, b1, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    // TODO: MOO add shard tracking.
    //    assert!(chain.process_block(&None, b2, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_err());
}

#[test]
fn test_tx_wrong_fork() {
    init_test_logger();
    let (mut chain, _, signer) = setup();
    let genesis = chain.get_block_by_height(0).unwrap();
    let b1 = Block::empty(genesis, signer.clone());
    let tx = SignedTransaction::new(
        Signature::empty(KeyType::ED25519),
        Transaction {
            signer_id: "".to_string(),
            public_key: signer.public_key(),
            nonce: 0,
            receiver_id: "".to_string(),
            block_hash: hash(&[2]),
            actions: vec![],
        },
    );
    let _b2 = Block::produce(
        &genesis.header,
        2,
        genesis.chunks.clone(),
        EpochId::default(),
        vec![tx],
        HashMap::default(),
        0,
        Some(0),
        signer.clone(),
    );
    assert!(chain.process_block(&None, b1, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_ok());
    // TODO: MOO add shard tracking.
    //    assert!(chain.process_block(&None, b2, Provenance::PRODUCED, |_, _, _| {}, |_| {}).is_err());
}

/// Verifies that the block at height are updated correctly when blocks from different forks are
/// processed, especially when certain heights are skipped
#[test]
fn blocks_at_height() {
    init_test_logger();
    let (mut chain, _, signer) = setup();
    let genesis = chain.get_block_by_height(0).unwrap();
    let b_1 = Block::empty_with_height(genesis, 1, signer.clone());
    let b_2 = Block::empty_with_height(&b_1, 2, signer.clone());
    let b_3 = Block::empty_with_height(&b_2, 3, signer.clone());

    let c_1 = Block::empty_with_height(&genesis, 1, signer.clone());
    let c_3 = Block::empty_with_height(&c_1, 3, signer.clone());
    let c_4 = Block::empty_with_height(&c_3, 4, signer.clone());
    let c_5 = Block::empty_with_height(&c_4, 5, signer.clone());

    let d_3 = Block::empty_with_height(&b_2, 3, signer.clone());
    let d_4 = Block::empty_with_height(&d_3, 4, signer.clone());
    let d_5 = Block::empty_with_height(&d_4, 5, signer.clone());

    let mut e_2 = Block::empty_with_height(&b_1, 2, signer.clone());
    e_2.header.inner.total_weight = (10 * e_2.header.inner.total_weight.to_num()).into();
    e_2.header.init();
    e_2.header.signature = signer.sign(e_2.header.hash().as_ref());

    let b_1_hash = b_1.hash();
    let b_2_hash = b_2.hash();
    let b_3_hash = b_3.hash();

    let c_1_hash = c_1.hash();
    let c_3_hash = c_3.hash();
    let c_4_hash = c_4.hash();
    let c_5_hash = c_5.hash();

    let d_3_hash = d_3.hash();
    let d_4_hash = d_4.hash();
    let d_5_hash = d_5.hash();

    let e_2_hash = e_2.hash();

    assert_ne!(d_3_hash, b_3_hash);

    chain.process_block(&None, b_1, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, b_2, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, b_3, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    assert_eq!(chain.header_head().unwrap().height, 3);

    assert_eq!(chain.get_header_by_height(1).unwrap().hash(), b_1_hash);
    assert_eq!(chain.get_header_by_height(2).unwrap().hash(), b_2_hash);
    assert_eq!(chain.get_header_by_height(3).unwrap().hash(), b_3_hash);

    chain.process_block(&None, c_1, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, c_3, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, c_4, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, c_5, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    assert_eq!(chain.header_head().unwrap().height, 5);

    assert_eq!(chain.get_header_by_height(1).unwrap().hash(), c_1_hash);
    assert!(chain.get_header_by_height(2).is_err());
    assert_eq!(chain.get_header_by_height(3).unwrap().hash(), c_3_hash);
    assert_eq!(chain.get_header_by_height(4).unwrap().hash(), c_4_hash);
    assert_eq!(chain.get_header_by_height(5).unwrap().hash(), c_5_hash);

    chain.process_block(&None, d_3, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, d_4, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    chain.process_block(&None, d_5, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    assert_eq!(chain.header_head().unwrap().height, 5);

    assert_eq!(chain.get_header_by_height(1).unwrap().hash(), b_1_hash);
    assert_eq!(chain.get_header_by_height(2).unwrap().hash(), b_2_hash);
    assert_eq!(chain.get_header_by_height(3).unwrap().hash(), d_3_hash);
    assert_eq!(chain.get_header_by_height(4).unwrap().hash(), d_4_hash);
    assert_eq!(chain.get_header_by_height(5).unwrap().hash(), d_5_hash);

    chain.process_block(&None, e_2, Provenance::PRODUCED, |_, _, _| {}, |_| {}).unwrap();
    assert_eq!(chain.header_head().unwrap().height, 2);

    assert_eq!(chain.get_header_by_height(1).unwrap().hash(), b_1_hash);
    assert_eq!(chain.get_header_by_height(2).unwrap().hash(), e_2_hash);
    assert!(chain.get_header_by_height(3).is_err());
    assert!(chain.get_header_by_height(4).is_err());
    assert!(chain.get_header_by_height(5).is_err());
}
