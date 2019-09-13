use crate::dependencies::ExternalError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    BadUTF16,
    BadUTF8,
    GasExceeded,
    GasLimitExceeded,
    BalanceExceeded,
    EmptyMethodName,
    GuestPanic,
    IntegerOverflow,
    InvalidPromiseIndex,
    CannotAppendActionToJointPromise,
    CannotReturnJointPromise,
    InvalidPromiseResultIndex,
    InvalidRegisterId,
    IteratorWasInvalidated,
    MemoryAccessViolation,
    InvalidReceiptIndex,
    InvalidIteratorIndex,
    InvalidAccountId,
    InvalidMethodName,
    InvalidPublicKey,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostErrorOrStorageError {
    HostError(HostError),
    /// Error from underlying storage, serialized
    StorageError(Vec<u8>),
}

impl From<HostError> for HostErrorOrStorageError {
    fn from(err: HostError) -> Self {
        HostErrorOrStorageError::HostError(err)
    }
}

impl From<ExternalError> for HostErrorOrStorageError {
    fn from(err: ExternalError) -> Self {
        match err {
            ExternalError::InvalidReceiptIndex => HostError::InvalidReceiptIndex.into(),
            ExternalError::InvalidIteratorIndex => HostError::InvalidIteratorIndex.into(),
            ExternalError::InvalidAccountId => HostError::InvalidAccountId.into(),
            ExternalError::InvalidMethodName => HostError::InvalidMethodName.into(),
            ExternalError::InvalidPublicKey => HostError::InvalidPublicKey.into(),
            ExternalError::StorageError(e) => HostErrorOrStorageError::StorageError(e),
        }
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        use HostError::*;
        match self {
            BadUTF8 => write!(f, "String encoding is bad UTF-8 sequence."),
            BadUTF16 => write!(f, "String encoding is bad UTF-16 sequence."),
            GasExceeded => write!(f, "Exceeded the prepaid gas."),
            GasLimitExceeded => write!(f, "Exceeded the maximum amount of gas allowed to burn per contract."),
            BalanceExceeded => write!(f, "Exceeded the account balance."),
            EmptyMethodName => write!(f, "Tried to call an empty method name."),
            GuestPanic => write!(f, "Smart contract has explicitly invoked `panic`."),
            IntegerOverflow => write!(f, "Integer overflow."),
            InvalidIteratorIndex => write!(f, "Invalid iterator index"),
            InvalidPromiseIndex => write!(f, "Invalid promise index"),
            CannotAppendActionToJointPromise => write!(f, "Actions can only be appended to non-joint promise."),
            CannotReturnJointPromise => write!(f, "Returning joint promise is currently prohibited."),
            InvalidPromiseResultIndex => write!(f, "Accessed invalid promise result index."),
            InvalidRegisterId => write!(f, "Accessed invalid register id"),
            IteratorWasInvalidated => write!(f, "Iterator was invalidated after its creation by performing a mutable operation on trie"),
            MemoryAccessViolation => write!(f, "Accessed memory outside the bounds."),
            InvalidReceiptIndex => write!(f, "VM Logic returned an invalid receipt index"),
            InvalidAccountId => write!(f, "VM Logic returned an invalid account id"),
            InvalidMethodName => write!(f, "VM Logic returned an invalid method name"),
            InvalidPublicKey => write!(f, "VM Logic provided an invalid public key"),
//            StorageError(e) => write!(f, "Storage error: {:?}", e),
        }
    }
}
