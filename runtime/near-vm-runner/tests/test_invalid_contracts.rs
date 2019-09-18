use crate::utils::{make_simple_contract_call, wat2wasm_no_validate};
use near_vm_runner::errors::{CompilationError, FunctionCallError, PrepareError};
use near_vm_runner::VMError;

mod utils;

fn initializer_wrong_signature_contract() -> Vec<u8> {
    wat2wasm_no_validate(
        r#"
            (module
              (type (;0;) (func (param i32)))
              (func (;0;) (type 0))
              (start 0)
              (export "hello" (func 0))
            )"#,
    )
}

#[test]
fn test_initializer_wrong_signature_contract() {
    assert_eq!(
        make_simple_contract_call(&initializer_wrong_signature_contract(), b"hello"),
        ((
            None,
            Some(VMError::FunctionCallError(FunctionCallError::CompilationError(
                CompilationError::WasmerCompileError(
                    "Validation error \"invlid start function type\"".to_string()
                )
            )))
        ))
    );
}

fn function_not_defined_contract() -> Vec<u8> {
    wat2wasm_no_validate(
        r#"
            (module
              (export "hello" (func 0))
            )"#,
    )
}

#[test]
/// StackHeightInstrumentation is weird but it's what we return for now
fn test_function_not_defined_contract() {
    assert_eq!(
        make_simple_contract_call(&function_not_defined_contract(), b"hello"),
        ((
            None,
            Some(VMError::FunctionCallError(FunctionCallError::CompilationError(
                CompilationError::PrepareError(PrepareError::StackHeightInstrumentation)
            )))
        ))
    );
}

#[test]
fn test_garbage_contract() {
    assert_eq!(
        make_simple_contract_call(&[], b"hello"),
        ((
            None,
            Some(VMError::FunctionCallError(FunctionCallError::CompilationError(
                CompilationError::PrepareError(PrepareError::Deserialization)
            )))
        ))
    );
}
