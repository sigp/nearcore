use crate::utils::make_simple_contract_call;
use near_vm_errors::{CompilationError, FunctionCallError, MethodResolveError, PrepareError};
use near_vm_logic::{HostError, ReturnData, VMOutcome};
use near_vm_runner::VMError;

mod utils;

fn vm_outcome_with_gas(gas: u64) -> VMOutcome {
    VMOutcome {
        balance: 0,
        storage_usage: 0,
        return_data: ReturnData::None,
        burnt_gas: gas,
        used_gas: gas,
        logs: vec![],
    }
}

fn infinite_initializer_contract() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (func (;0;) (type 0) (loop (br 0)))
              (func (;1;) (type 0))
              (start 0)
              (export "hello" (func 1))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_infinite_initializer() {
    assert_eq!(
        make_simple_contract_call(&infinite_initializer_contract(), b"hello"),
        (
            Some(vm_outcome_with_gas(1000000)),
            Some(VMError::FunctionCallError(FunctionCallError::HostError(HostError::GasExceeded)))
        )
    );
}

#[test]
// Current behavior is to run the initializer even if the method doesn't exist
fn test_infinite_initializer_export_not_found() {
    assert_eq!(
        make_simple_contract_call(&infinite_initializer_contract(), b"hello2"),
        (
            Some(vm_outcome_with_gas(1000000)),
            Some(VMError::FunctionCallError(FunctionCallError::HostError(HostError::GasExceeded)))
        )
    );
}

fn simple_contract() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (func (;0;) (type 0))
              (export "hello" (func 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_simple_contract() {
    assert_eq!(
        make_simple_contract_call(&simple_contract(), b"hello"),
        (Some(vm_outcome_with_gas(1)), None)
    );
}

#[test]
// Current behavior is to return a VMOutcome even if there is no initializer and no method
fn test_export_not_found() {
    assert_eq!(
        make_simple_contract_call(&simple_contract(), b"hello2"),
        (
            Some(vm_outcome_with_gas(0)),
            Some(VMError::FunctionCallError(FunctionCallError::ResolveError(
                MethodResolveError::MethodNotFound
            )))
        )
    );
}

#[test]
fn test_empty_method() {
    assert_eq!(
        make_simple_contract_call(&simple_contract(), b""),
        (
            None,
            Some(VMError::FunctionCallError(FunctionCallError::ResolveError(
                MethodResolveError::MethodEmptyName
            )))
        )
    );
}

#[test]
fn test_invalid_utf8() {
    assert_eq!(
        make_simple_contract_call(&simple_contract(), &[255u8]),
        (
            None,
            Some(VMError::FunctionCallError(FunctionCallError::ResolveError(
                MethodResolveError::MethodUTF8Error
            )))
        )
    );
}

fn trap_contract() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (func (;0;) (type 0) (unreachable))
              (export "hello" (func 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_trap_contract() {
    assert_eq!(
        make_simple_contract_call(&trap_contract(), b"hello"),
        (
            Some(vm_outcome_with_gas(2)),
            Some(VMError::FunctionCallError(FunctionCallError::WasmTrap("unknown".to_string())))
        )
    );
}

fn trap_initializer() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (func (;0;) (type 0) (unreachable))
              (start 0)
              (export "hello" (func 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_trap_initializer() {
    assert_eq!(
        make_simple_contract_call(&trap_initializer(), b"hello"),
        (
            Some(vm_outcome_with_gas(2)),
            Some(VMError::FunctionCallError(FunctionCallError::WasmTrap("unknown".to_string())))
        )
    );
}

fn wrong_signature_contract() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func (param i32)))
              (func (;0;) (type 0))
              (export "hello" (func 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_wrong_signature_contract() {
    assert_eq!(
        make_simple_contract_call(&wrong_signature_contract(), b"hello"),
        (
            Some(vm_outcome_with_gas(0)),
            Some(VMError::FunctionCallError(FunctionCallError::ResolveError(
                MethodResolveError::MethodInvalidSignature
            )))
        )
    );
}

fn export_wrong_type() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (global (;0;) i32 (i32.const 123))
              (export "hello" (global 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_export_wrong_type() {
    assert_eq!(
        make_simple_contract_call(&export_wrong_type(), b"hello"),
        (
            Some(vm_outcome_with_gas(0)),
            Some(VMError::FunctionCallError(FunctionCallError::ResolveError(
                MethodResolveError::MethodNotFound
            )))
        )
    );
}

fn guest_panic() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (import "env" "panic" (func (;0;) (type 0)))
              (func (;1;) (type 0) (call 0))
              (export "hello" (func 1))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_guest_panic() {
    assert_eq!(
        make_simple_contract_call(&guest_panic(), b"hello"),
        (
            Some(vm_outcome_with_gas(2)),
            Some(VMError::FunctionCallError(FunctionCallError::HostError(HostError::GuestPanic)))
        )
    );
}

fn stack_overflow() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (func (;0;) (type 0) (call 0))
              (export "hello" (func 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_stack_overflow() {
    assert_eq!(
        make_simple_contract_call(&stack_overflow(), b"hello"),
        (
            Some(vm_outcome_with_gas(129870)),
            Some(VMError::FunctionCallError(FunctionCallError::WasmTrap("unknown".to_string())))
        )
    );
}

fn memory_grow() -> Vec<u8> {
    wabt::wat2wasm(
        r#"
            (module
              (type (;0;) (func))
              (func (;0;) (type 0)
                (loop
                  (memory.grow (i32.const 1))
                  drop
                  br 0
                )
              )
              (memory (;0;) 17 32)
              (export "hello" (func 0))
            )"#,
    )
    .unwrap()
}

#[test]
fn test_memory_grow() {
    assert_eq!(
        make_simple_contract_call(&memory_grow(), b"hello"),
        (
            Some(vm_outcome_with_gas(1000000)),
            Some(VMError::FunctionCallError(FunctionCallError::HostError(HostError::GasExceeded)))
        )
    );
}

fn bad_import_global(env: &str) -> Vec<u8> {
    wabt::wat2wasm(format!(
        r#"
            (module
              (type (;0;) (func))
              (import "{}" "input" (global (;0;) i32))
              (func (;0;) (type 0))
              (export "hello" (func 0))
            )"#,
        env
    ))
    .unwrap()
}

fn bad_import_func(env: &str) -> Vec<u8> {
    wabt::wat2wasm(format!(
        r#"
            (module
              (type (;0;) (func))
              (import "{}" "wtf" (func (;0;) (type 0)))
              (func (;0;) (type 0))
              (export "hello" (func 0))
            )"#,
        env
    ))
    .unwrap()
}

#[test]
// Weird behavior:
// Invalid import not from "env" -> PrepareError::Instantiate
// Invalid import from "env" -> LinkError
fn test_bad_import_1() {
    assert_eq!(
        make_simple_contract_call(&bad_import_global("wtf"), b"hello"),
        (
            None,
            Some(VMError::FunctionCallError(FunctionCallError::CompilationError(
                CompilationError::PrepareError(PrepareError::Instantiate)
            )))
        )
    );
}

#[test]
fn test_bad_import_2() {
    assert_eq!(
        make_simple_contract_call(&bad_import_func("wtf"), b"hello"),
        (
            None,
            Some(VMError::FunctionCallError(FunctionCallError::CompilationError(
                CompilationError::PrepareError(PrepareError::Instantiate)
            )))
        )
    );
}

#[test]
fn test_bad_import_3() {
    assert_eq!(
        make_simple_contract_call(&bad_import_global("env"), b"hello"),
        (
            Some(vm_outcome_with_gas(0)),
            Some(VMError::FunctionCallError(FunctionCallError::LinkError(
                "link error: Incorrect import type, namespace: env, name: input, expected type: global, found type: function".to_string()
            )))
        )
    );
}

#[test]
fn test_bad_import_4() {
    assert_eq!(
        make_simple_contract_call(&bad_import_func("env"), b"hello"),
        (
            Some(vm_outcome_with_gas(0)),
            Some(VMError::FunctionCallError(FunctionCallError::LinkError(
                "link error: Import not found, namespace: env, name: wtf".to_string()
            )))
        )
    );
}
