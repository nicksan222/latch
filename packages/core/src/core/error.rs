//! The one error type of the vault core.
//!
//! Messages are shown to users and written to logs, so they must never contain
//! secret values. Callers construct messages from ids, paths, field names and
//! static text only. Decryption/deserialization failures of encrypted documents
//! are mapped to static messages (serde errors can quote input values).

use crate::protocol::ErrorCode;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("vault is locked")]
    Locked,
    #[error("no vault at {0}")]
    NoVault(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0} already exists")]
    AlreadyExists(String),
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("wrong passphrase or identity")]
    BadCredentials,
    #[error("integrity error: {0}")]
    Integrity(String),
    #[error("sync conflict: {0}")]
    Conflict(String),
    #[error("denied: {0}")]
    Denied(String),
    #[error("vault format version {found} is newer than supported version {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("git: {0}")]
    Git(String),
    #[error("io error ({context}): {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    pub fn code(&self) -> ErrorCode {
        match self {
            Error::Locked => ErrorCode::Locked,
            Error::NoVault(_) => ErrorCode::NoVault,
            Error::NotFound(_) => ErrorCode::NotFound,
            Error::AlreadyExists(_) => ErrorCode::AlreadyExists,
            Error::InvalidParams(_) => ErrorCode::InvalidParams,
            Error::BadCredentials => ErrorCode::BadCredentials,
            Error::Integrity(_) => ErrorCode::Integrity,
            Error::Conflict(_) => ErrorCode::Conflict,
            Error::Denied(_) => ErrorCode::Denied,
            Error::UnsupportedVersion { .. } => ErrorCode::UnsupportedVersion,
            Error::Git(_) => ErrorCode::Git,
            Error::Io { .. } => ErrorCode::Io,
            Error::Internal(_) => ErrorCode::Internal,
        }
    }

    /// Convert to the wire error.
    pub fn to_rpc(&self) -> crate::protocol::RpcError {
        crate::protocol::RpcError {
            code: self.code(),
            message: self.to_string(),
        }
    }

    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }

    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
        Error::InvalidParams(msg.into())
    }

    pub(crate) fn integrity(msg: impl Into<String>) -> Self {
        Error::Integrity(msg.into())
    }
}

impl From<Error> for crate::protocol::RpcError {
    fn from(e: Error) -> Self {
        e.to_rpc()
    }
}

/// Extension to attach a path/context to io errors.
pub(crate) trait IoContext<T> {
    fn ctx(self, context: impl FnOnce() -> String) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn ctx(self, context: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|e| Error::io(context(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_map() {
        assert_eq!(Error::Locked.code(), ErrorCode::Locked);
        assert_eq!(Error::BadCredentials.code(), ErrorCode::BadCredentials);
        assert_eq!(Error::integrity("x").code(), ErrorCode::Integrity);
        assert_eq!(
            Error::UnsupportedVersion {
                found: 2,
                supported: 1
            }
            .code(),
            ErrorCode::UnsupportedVersion
        );
        assert_eq!(Error::Git("x".into()).code(), ErrorCode::Git);
        let e = Error::io("read", std::io::Error::other("boom"));
        assert_eq!(e.code(), ErrorCode::Io);
        assert_eq!(e.to_rpc().code, ErrorCode::Io);
        assert!(e.to_string().contains("read"));
    }
}
