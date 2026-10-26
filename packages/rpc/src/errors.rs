//! Domain error details carried by standard gRPC statuses.
use crate::v1::{self, ErrorCode};
use prost::Message;

#[derive(Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: ErrorCode,
    pub message: String,
}
impl RpcError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidParams,
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: message.into(),
        }
    }
}
impl std::fmt::Debug for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcError")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error ({:?})", self.code)
    }
}
impl std::error::Error for RpcError {}

pub fn status(error: &RpcError) -> tonic::Status {
    let detail = v1::RpcError {
        code: error.code as i32,
        message: error.message.clone(),
    };
    let code = match error.code {
        ErrorCode::Locked | ErrorCode::NoVault | ErrorCode::UnsupportedVersion => {
            tonic::Code::FailedPrecondition
        }
        ErrorCode::NotFound => tonic::Code::NotFound,
        ErrorCode::AlreadyExists => tonic::Code::AlreadyExists,
        ErrorCode::InvalidParams => tonic::Code::InvalidArgument,
        ErrorCode::BadCredentials => tonic::Code::Unauthenticated,
        ErrorCode::Integrity => tonic::Code::DataLoss,
        ErrorCode::Conflict => tonic::Code::Aborted,
        ErrorCode::Denied => tonic::Code::PermissionDenied,
        ErrorCode::Io => tonic::Code::Unavailable,
        ErrorCode::Unspecified | ErrorCode::Git | ErrorCode::Internal => tonic::Code::Internal,
    };
    tonic::Status::with_details(code, "Latch RPC error", detail.encode_to_vec().into())
}
pub fn rpc_error(status: tonic::Status) -> RpcError {
    if !status.details().is_empty()
        && let Ok(detail) = v1::RpcError::decode(status.details())
    {
        let code = ErrorCode::try_from(detail.code)
            .ok()
            .filter(|c| *c != ErrorCode::Unspecified)
            .unwrap_or(ErrorCode::Internal);
        return RpcError {
            code,
            message: detail.message,
        };
    }
    let (code, message) = match status.code() {
        tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => {
            (ErrorCode::Denied, "RPC access denied")
        }
        tonic::Code::Unavailable | tonic::Code::DeadlineExceeded => {
            (ErrorCode::Io, "agent unavailable")
        }
        tonic::Code::InvalidArgument => (ErrorCode::InvalidParams, "invalid RPC parameters"),
        _ => (ErrorCode::Internal, "RPC transport failed"),
    };
    RpcError {
        code,
        message: message.into(),
    }
}
