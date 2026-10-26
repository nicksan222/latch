//! The single `rpc` command: the only path from the webview to vault data.
//!
//! [`handle_rpc`] is window-free so it can be tested directly; the Tauri
//! command moves it onto the blocking pool and applies shell settings.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use super::{Shell, apply_settings};

use latch::rpc::{Agent, ClientCtx, LocalClient};
use latch_rpc::{
    sdk::methods as m,
    types::{ErrorCode, RpcError, Settings},
};

/// What the `rpc` command rejects with. Serializes as `{ code, message }`,
/// the shape `src/lib/api/errors.ts` (`RpcError.from`) expects.
pub type RpcErrorPayload = RpcError;

#[tauri::command]
pub async fn rpc(
    app: AppHandle,
    shell: State<'_, Shell>,
    method: String,
    params: Option<Value>,
) -> Result<Value, RpcErrorPayload> {
    let client = shell.rpc_client.clone();
    let m2 = method.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        client
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .call(&m2, params.unwrap_or(Value::Null))
    })
    .await
    .map_err(|_| internal_error("request failed"))?;
    if method == m::SETTINGS_UPDATE
        && let Ok(v) = &res
        && let Ok(s) = serde_json::from_value::<Settings>(v.clone())
    {
        apply_settings(&app, &s);
    }
    res
}

/// Dispatch one request as the desktop client. Blocking (KDF, git).
pub fn handle_rpc(
    agent: &Arc<Agent>,
    method: &str,
    params: Value,
) -> Result<Value, RpcErrorPayload> {
    LocalClient::new(agent, ClientCtx::desktop()).call(method, params)
}

/// Error used when the blocking task itself failed (panic/cancellation).
pub fn internal_error(message: &str) -> RpcErrorPayload {
    RpcError {
        code: ErrorCode::Internal,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn error_payload_shape_is_code_and_message() {
        let e = internal_error("boom");
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            json!({ "code": "internal", "message": "boom" })
        );
        let e = RpcError {
            code: ErrorCode::BadCredentials,
            message: "wrong passphrase".into(),
        };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            json!({ "code": "bad_credentials", "message": "wrong passphrase" })
        );
    }

    #[test]
    fn bridge_preserves_core_rpc_errors() {
        // No vault/config is opened: a dynamic unknown procedure is rejected by core.
        let directory = tempfile::tempdir().unwrap();
        let agent = Agent::new(latch::rpc::AgentOptions {
            config_dir: Some(directory.path().join("config")),
            clipboard: Some(Arc::new(latch::agent::clipboard::MemoryClipboard::new())),
            browser_config: Some(latch::agent::browser::ManagerConfig {
                ram_dir: directory.path().join("ram"),
                data_dir: directory.path().join("data"),
                browser_override: None,
                path_var: Some(directory.path().join("empty-path").into_os_string()),
                term_timeout: std::time::Duration::from_millis(50),
                archive_wait: std::time::Duration::from_millis(50),
            }),
            watch_screen_lock: false,
            ..Default::default()
        });
        let error = handle_rpc(&agent, "desktop.unknown", Value::Null).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        agent.shutdown();
    }
}
