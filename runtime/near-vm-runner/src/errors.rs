use near_vm_logic::{ExternalError, HostError, HostErrorOrStorageError};
use std::fmt;
use wasmer_runtime::error::{
    CallError, CompileError, CreationError, ResolveError as WasmerResolveError,
    RuntimeError as WasmerRuntimeError, RuntimeError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VMError {
    FunctionCallError(FunctionCallError),
    StorageError(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionCallError {
    CompilationError(CompilationError),
    LinkError(String),
    ResolveError(MethodResolveError),
    WasmTrap(String),
    HostError(HostError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodResolveError {
    MethodEmptyName,
    MethodUTF8Error,
    MethodNotFound,
    MethodInvalidSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationError {
    CodeDoesNotExist(String),
    PrepareError(PrepareError),
    WasmerCompileError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Error that can occur while preparing or executing Wasm smart-contract.
pub enum PrepareError {
    /// Error happened while serializing the module.
    Serialization,

    /// Error happened while deserializing the module.
    Deserialization,

    /// Internal memory declaration has been found in the module.
    InternalMemoryDeclared,

    /// Gas instrumentation failed.
    ///
    /// This most likely indicates the module isn't valid.
    GasInstrumentation,

    /// Stack instrumentation failed.
    ///
    /// This  most likely indicates the module isn't valid.
    StackHeightInstrumentation,

    /// Error happened during instantiation.
    ///
    /// This might indicate that `start` function trapped, or module isn't
    /// instantiable and/or unlinkable.
    Instantiate,

    /// Error creating memory.
    Memory,
}

impl From<wasmer_runtime::error::Error> for VMError {
    fn from(err: wasmer_runtime::error::Error) -> Self {
        use wasmer_runtime::error::Error;
        match err {
            Error::CompileError(err) => err.into(),
            Error::LinkError(err) => VMError::FunctionCallError(FunctionCallError::LinkError(
                format!("{}", Error::LinkError(err)),
            )),
            Error::RuntimeError(err) => err.into(),
            Error::ResolveError(err) => err.into(),
            Error::CallError(err) => err.into(),
            Error::CreationError(err) => panic!(err),
        }
    }
}

impl From<CallError> for VMError {
    fn from(err: CallError) -> Self {
        match err {
            CallError::Resolve(err) => err.into(),
            CallError::Runtime(err) => err.into(),
        }
    }
}

impl From<CompileError> for VMError {
    fn from(err: CompileError) -> Self {
        VMError::FunctionCallError(FunctionCallError::CompilationError(
            CompilationError::WasmerCompileError(err.to_string()),
        ))
    }
}

impl From<WasmerResolveError> for VMError {
    fn from(err: WasmerResolveError) -> Self {
        match err {
            WasmerResolveError::Signature { .. } => VMError::FunctionCallError(
                FunctionCallError::ResolveError(MethodResolveError::MethodInvalidSignature),
            ),
            WasmerResolveError::ExportNotFound { .. } => VMError::FunctionCallError(
                FunctionCallError::ResolveError(MethodResolveError::MethodNotFound),
            ),
            WasmerResolveError::ExportWrongType { .. } => VMError::FunctionCallError(
                FunctionCallError::ResolveError(MethodResolveError::MethodNotFound),
            ),
        }
    }
}

impl From<RuntimeError> for VMError {
    fn from(err: WasmerRuntimeError) -> Self {
        match &err {
            WasmerRuntimeError::Trap { msg } => {
                VMError::FunctionCallError(FunctionCallError::WasmTrap(msg.to_string()))
            }
            WasmerRuntimeError::Error { data } => {
                if let Some(err) = data.downcast_ref::<HostErrorOrStorageError>() {
                    match err {
                        HostErrorOrStorageError::StorageError(s) => {
                            VMError::StorageError(s.clone())
                        }
                        HostErrorOrStorageError::HostError(h) => {
                            VMError::FunctionCallError(FunctionCallError::HostError(h.clone()))
                        }
                    }
                } else {
                    eprintln!(
                        "Bad error case! Output is non-deterministic {:?} {:?}",
                        data.type_id(),
                        err.to_string()
                    );
                    VMError::FunctionCallError(FunctionCallError::WasmTrap("unknown".to_string()))
                }
            }
        }
    }
}

impl From<PrepareError> for VMError {
    fn from(err: PrepareError) -> Self {
        VMError::FunctionCallError(FunctionCallError::CompilationError(
            CompilationError::PrepareError(err),
        ))
    }
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        use PrepareError::*;
        match self {
            Serialization => write!(f, "Error happened while serializing the module."),
            Deserialization => write!(f, "Error happened while deserializing the module."),
            InternalMemoryDeclared => {
                write!(f, "Internal memory declaration has been found in the module.")
            }
            GasInstrumentation => write!(f, "Gas instrumentation failed."),
            StackHeightInstrumentation => write!(f, "Stack instrumentation failed."),
            Instantiate => write!(f, "Error happened during instantiation."),
            Memory => write!(f, "Error creating memory"),
        }
    }
}

impl fmt::Display for FunctionCallError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            FunctionCallError::CompilationError(e) => e.fmt(f),
            FunctionCallError::ResolveError(e) => e.fmt(f),
            FunctionCallError::HostError(e) => e.fmt(f),
            FunctionCallError::LinkError(s) => write!(f, "{}", s),
            FunctionCallError::WasmTrap(s) => write!(f, "WebAssembly trap: {}", s),
        }
    }
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            CompilationError::CodeDoesNotExist(account_id) => {
                write!(f, "cannot find contract code for account {}", account_id)
            }
            CompilationError::PrepareError(p) => write!(f, "PrepareError: {}", p),
            CompilationError::WasmerCompileError(s) => write!(f, "Wasmer compilation error: {}", s),
        }
    }
}

impl fmt::Display for MethodResolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Display for VMError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}
