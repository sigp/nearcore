use near_vm_logic::mocks::mock_external::MockedExternal;
use near_vm_logic::{Config, VMContext, VMOutcome};
use near_vm_runner::{run, VMError};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use wabt::Wat2Wasm;

const CURRENT_ACCOUNT_ID: &str = "alice";
const SIGNER_ACCOUNT_ID: &str = "bob";
const SIGNER_ACCOUNT_PK: [u8; 3] = [0, 1, 2];
const PREDECESSOR_ACCOUNT_ID: &str = "carol";

pub fn create_context(input: Vec<u8>) -> VMContext {
    VMContext {
        current_account_id: CURRENT_ACCOUNT_ID.to_owned(),
        signer_account_id: SIGNER_ACCOUNT_ID.to_owned(),
        signer_account_pk: Vec::from(&SIGNER_ACCOUNT_PK[..]),
        predecessor_account_id: PREDECESSOR_ACCOUNT_ID.to_owned(),
        input,
        block_index: 0,
        account_balance: 0,
        storage_usage: 0,
        attached_deposit: 0,
        prepaid_gas: 10u64.pow(9),
        random_seed: vec![0, 1, 2],
        free_of_charge: false,
        output_data_receivers: vec![],
    }
}

pub fn make_simple_contract_call(
    code: &[u8],
    method_name: &[u8],
) -> (Option<VMOutcome>, Option<VMError>) {
    let mut fake_external = MockedExternal::new();
    let mut context = create_context(vec![]);
    context.prepaid_gas = 10u64.pow(6);
    let config = Config::default();

    let promise_results = vec![];

    let mut hash = DefaultHasher::new();
    code.hash(&mut hash);
    let code_hash = hash.finish().to_le_bytes().to_vec();
    run(code_hash, code, method_name, &mut fake_external, context, &config, &promise_results)
}

pub fn wat2wasm_no_validate(wat: &str) -> Vec<u8> {
    Wat2Wasm::new().validate(false).convert(wat).unwrap().as_ref().to_vec()
}
