//! Authentication metadata and typed calls for the generated tonic client.
use crate::{errors::RpcError, v1};

pub fn request<T>(message: T, identity: &v1::HelloParams) -> Result<tonic::Request<T>, RpcError> {
    let kind = match v1::ClientKind::try_from(identity.client) {
        Ok(v1::ClientKind::Cli) => "cli",
        Ok(v1::ClientKind::Desktop) => "desktop",
        _ => return Err(RpcError::invalid("invalid client kind")),
    };
    if identity.version.is_empty() {
        return Err(RpcError::invalid("missing client version"));
    }
    let mut request = tonic::Request::new(message);
    request.metadata_mut().insert(
        "x-latch-client",
        kind.parse()
            .map_err(|_| RpcError::invalid("invalid client kind"))?,
    );
    request.metadata_mut().insert(
        "x-latch-version",
        identity
            .version
            .parse()
            .map_err(|_| RpcError::invalid("invalid client version"))?,
    );
    Ok(request)
}
include!(concat!(env!("OUT_DIR"), "/latch_client.rs"));
