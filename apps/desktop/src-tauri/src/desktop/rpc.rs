//! Protobuf tunnel from the webview to the embedded agent.

use super::{Shell, apply_settings};
use latch::rpc::{Agent, ClientCtx, LocalClient};
use latch_rpc::{
    errors::RpcError,
    sdk::{Call, Reply},
    v1::ErrorCode,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize)]
pub struct RpcErrorPayload {
    code: i32,
    message: String,
}

impl From<RpcError> for RpcErrorPayload {
    fn from(error: RpcError) -> Self {
        Self {
            code: error.code as i32,
            message: error.message,
        }
    }
}

#[tauri::command]
pub async fn grpc_request(
    app: AppHandle,
    shell: State<'_, Shell>,
    method: String,
    payload: Vec<u8>,
) -> Result<Vec<u8>, RpcErrorPayload> {
    let call = Call::decode(&method, &payload)?;
    let client = shell.rpc_client.clone();
    let reply = tauri::async_runtime::spawn_blocking(move || {
        client
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .execute(call)
    })
    .await
    .map_err(|_| internal_error("request failed"))??;
    if let Reply::SettingsUpdate(settings) = &reply {
        apply_settings(&app, settings);
    }
    Ok(reply.encode())
}

/// Dispatch one protobuf request as the desktop client. Blocking (KDF, Git).
pub fn handle_rpc(agent: &Arc<Agent>, method: &str, payload: &[u8]) -> Result<Vec<u8>, RpcError> {
    let call = Call::decode(method, payload)?;
    LocalClient::new(agent, ClientCtx::desktop())
        .execute(call)
        .map(|reply| reply.encode())
}

pub fn internal_error(message: &str) -> RpcErrorPayload {
    RpcErrorPayload::from(RpcError {
        code: ErrorCode::Internal,
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_payload_shape_is_code_and_message() {
        assert_eq!(
            serde_json::to_value(internal_error("boom")).unwrap(),
            serde_json::json!({"code":ErrorCode::Internal as i32, "message":"boom"})
        );
        let error = RpcError {
            code: ErrorCode::BadCredentials,
            message: "wrong passphrase".into(),
        };
        assert_eq!(
            serde_json::to_value(RpcErrorPayload::from(error)).unwrap(),
            serde_json::json!({"code":ErrorCode::BadCredentials as i32, "message":"wrong passphrase"})
        );
    }
    #[test]
    fn rejects_unknown_and_malformed_calls() {
        assert!(Call::decode("desktop.unknown", &[]).is_err());
        assert!(Call::decode("VaultUnlock", &[0xff]).is_err());
    }
}
