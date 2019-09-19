use near_vm_errors::FunctionCallError;
use near_vm_logic::mocks::mock_external::MockedExternal;
use near_vm_logic::types::ReturnData;
use near_vm_logic::{Config, VMContext, VMOutcome};
use near_vm_runner::{run, VMError};
use std::fs;
use std::mem::size_of;
use std::path::PathBuf;

mod utils;

fn assert_run_result((outcome, err): (Option<VMOutcome>, Option<VMError>), expected_value: u64) {
    if let Some(_) = err {
        panic!("Failed execution");
    }

    if let Some(VMOutcome { return_data, .. }) = outcome {
        if let ReturnData::Value(value) = return_data {
            let mut arr = [0u8; size_of::<u64>()];
            arr.copy_from_slice(&value);
            let res = u64::from_le_bytes(arr);
            assert_eq!(res, expected_value);
        } else {
            panic!("Value was not returned");
        }
    } else {
        panic!("Failed execution");
    }
}

fn arr_u64_to_u8(value: &[u64]) -> Vec<u8> {
    let mut res = vec![];
    for el in value {
        res.extend_from_slice(&el.to_le_bytes());
    }
    res
}

fn create_context(input: &[u64]) -> VMContext {
    let input = arr_u64_to_u8(input);
    crate::utils::create_context(input)
}

#[test]
pub fn test_read_write() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/res/test_contract_rs.wasm");
    let code = fs::read(path).unwrap();
    let mut fake_external = MockedExternal::new();

    let context = create_context(&[10u64, 20u64]);
    let config = Config::default();

    let promise_results = vec![];
    let result = run(
        vec![],
        &code,
        b"write_key_value",
        &mut fake_external,
        context,
        &config,
        &promise_results,
    );
    assert_run_result(result, 0);

    let context = create_context(&[10u64]);
    let result =
        run(vec![], &code, b"read_value", &mut fake_external, context, &config, &promise_results);
    assert_run_result(result, 20);
}

#[test]
pub fn test_out_of_memory() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/res/test_contract_rs.wasm");
    let code = fs::read(path).unwrap();
    let mut fake_external = MockedExternal::new();

    let context = create_context(&[]);
    let config = Config::default();

    let promise_results = vec![];
    let result = run(
        vec![],
        &code,
        b"out_of_memory",
        &mut fake_external,
        context,
        &config,
        &promise_results,
    );
    assert_eq!(
        result.1,
        Some(VMError::FunctionCallError(FunctionCallError::WasmTrap("unknown".to_string())))
    );
}
