//! Private per-session agent worker. IPC is JSON lines over inherited private pipes.
use crate::{Options, fixture};
use latch::{
    agent::{
        Agent, AgentOptions, ClientCtx,
        browser::ManagerConfig,
        clipboard::{ClipboardBackend, MemoryClipboard},
    },
    protocol::{ErrorCode, RpcError, RpcRequest, RpcResponse},
};
use serde_json::{Value, json};
use std::time::Duration;
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
struct Session {
    agent: Arc<Agent>,
    rpc: std::sync::Mutex<latch::rpc::LocalClient>,
    clip: Arc<MemoryClipboard>,
    root: PathBuf,
    fixture: Value,
    inspection: std::sync::Mutex<Value>,
    sync_ready: std::sync::atomic::AtomicBool,
    sync_setup_attempted: std::sync::atomic::AtomicBool,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.agent.shutdown();
    }
}
fn inspect(s: &Session) -> Value {
    let mut v = s.inspection.lock().unwrap().clone();
    v["clipboard"] = json!(s.clip.get_text());
    v
}
fn denied(id: u64) -> RpcResponse {
    RpcResponse {
        id,
        result: None,
        error: Some(RpcError {
            code: ErrorCode::Denied,
            message: "operation unavailable outside fixture arena".into(),
        }),
    }
}
fn safe_path(s: &Session, raw: &str) -> Option<PathBuf> {
    let path = Path::new(raw);
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    if path.parent() == Some(s.root.as_path().join("files").as_path()) {
        return Some(path.to_owned());
    }
    // Arbitrary UI-selected filenames are virtual labels, never host paths.
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    raw.hash(&mut hash);
    Some(s.root.join("files").join(format!("{:016x}", hash.finish())))
}
fn adapt(s: &Session, req: &mut RpcRequest) -> bool {
    if req.params.is_null() {
        req.params = json!({});
    }
    if !req.params.is_object() {
        return false;
    }
    let p = &mut req.params;
    match req.method.as_str() {
        "vault.clone" | "sync.setRemote" | "vault.locate" | "doctor.scan" | "hooks.install"
        | "hooks.uninstall" => return false,
        "sync.run" | "sync.resolve" => {
            if !s.sync_ready.load(std::sync::atomic::Ordering::SeqCst)
                || s.agent.status().path.as_deref() != s.fixture["vaults"]["personal"].as_str()
            {
                return false;
            }
        }
        "recipients.add" => {
            let Some(key) = p["publicKey"].as_str() else {
                return false;
            };
            // Native age has one separator. Reject plugin/SSH syntax before
            // parsing, since plugin parsing can spawn an external executable.
            if !key.starts_with("age1")
                || key.len() != 62
                || key[4..].contains('1')
                || latch::core::crypto::parse_recipient(key).is_err()
            {
                return false;
            }
        }
        "vault.unlock" => {
            if p.get("identityFile").is_some_and(|v| !v.is_null()) {
                return false;
            }
        }
        "profiles.create" | "profiles.update" => {
            let input = if req.method == "profiles.update" {
                &mut p["input"]
            } else {
                &mut *p
            };
            if let Some(b) = input.get_mut("browser") {
                if !b.is_object() {
                    return false;
                }
                b["binary"] = json!(s.root.as_path().join("fixture-browser"));
            }
        }
        "import.preview" | "import.apply" => {
            if matches!(p["format"].as_str(), Some("pass")) {
                return false;
            }
        }
        _ => {}
    }
    for key in ["path", "destination", "outputPath"] {
        if let Some(raw) = p.get(key).and_then(Value::as_str) {
            let path = if matches!(req.method.as_str(), "vault.init" | "vault.open")
                && s.fixture["vaults"]
                    .as_object()
                    .is_some_and(|v| v.values().any(|x| x.as_str() == Some(raw)))
            {
                PathBuf::from(raw)
            } else {
                let Some(path) = safe_path(s, raw) else {
                    return false;
                };
                path
            };
            if std::fs::create_dir_all(path.parent().unwrap()).is_err() {
                return false;
            }
            p[key] = json!(path);
        }
    }
    let invalid_kdf = p.get("kdf").filter(|k| !k.is_null()).is_some_and(|k| {
        !k["memoryKib"]
            .as_u64()
            .is_some_and(|n| (65536..=262144).contains(&n))
            || !k["iterations"]
                .as_u64()
                .is_some_and(|n| (1..=5).contains(&n))
            || !k["parallelism"]
                .as_u64()
                .is_some_and(|n| (1..=4).contains(&n))
    });
    if matches!(req.method.as_str(), "vault.init" | "vault.changePassphrase") && invalid_kdf {
        return false;
    }
    if req.method == "vault.benchmarkKdf" && !p["targetMs"].as_u64().is_some_and(|n| n <= 1000) {
        return false;
    }
    if req.method == "vault.init" {
        p["remoteUrl"] = Value::Null;
        p["gitName"] = json!("Fixture");
        p["gitEmail"] = json!("fixture@example.invalid");
    }
    true
}

fn dispatch(s: &Session, mut request: RpcRequest) -> Value {
    let original = request.params.clone();
    let method = request.method.clone();
    let response = if method == "fixture.syncScenario" {
        let scenario = original["scenario"].as_str();
        if !original.as_object().is_some_and(|p| p.len() == 1)
            || !matches!(scenario, Some("conflict" | "remote_change"))
            || !s.agent.is_unlocked()
            || s.agent.status().path.as_deref() != s.fixture["vaults"]["personal"].as_str()
            || s.sync_setup_attempted
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            denied(request.id)
        } else {
            match fixture::sync_scenario(
                &s.agent,
                &s.root,
                &s.fixture,
                scenario == Some("conflict"),
            ) {
                Ok(result) => {
                    s.sync_ready
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    RpcResponse {
                        id: request.id,
                        result: Some(result),
                        error: None,
                    }
                }
                // A failed attempt can leave local Git state behind. Never
                // retry it or enable sync without a fresh fixture session.
                Err(()) => denied(request.id),
            }
        }
    } else if method == "browser.detect" {
        RpcResponse {
            id: request.id,
            result: Some(json!([])),
            error: None,
        }
    } else if adapt(s, &mut request) {
        match s.rpc.lock().unwrap().call(&request.method, request.params) {
            Ok(result) => RpcResponse {
                id: request.id,
                result: Some(result),
                error: None,
            },
            Err(error) => RpcResponse {
                id: request.id,
                result: None,
                error: Some(error),
            },
        }
    } else {
        denied(request.id)
    };
    if method == "export.run" && response.error.is_none() {
        let path = response.result.as_ref().and_then(|r| r["path"].as_str());
        let content = path
            .and_then(|p| std::fs::read(p).ok())
            .filter(|b| b.len() <= 2 * 1024 * 1024)
            .and_then(|b| String::from_utf8(b).ok());
        s.inspection.lock().unwrap()["lastExport"] =
            json!({"path":original["path"],"format":original["format"],"content":content});
    }

    json!({"response":response,"inspection":inspect(s)})
}
pub fn run(root: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut lines = std::io::stdin().lock().lines();
    let opts: Options = serde_json::from_str(&lines.next().ok_or("missing fixture options")??)?;
    let clip = Arc::new(MemoryClipboard::new());
    fixture::browser(&root)?;
    let cfg = ManagerConfig {
        ram_dir: root.join("ram"),
        data_dir: root.join("data"),
        browser_override: Some(root.join("fixture-browser")),
        path_var: Some("".into()),
        term_timeout: Duration::from_millis(100),
        archive_wait: Duration::from_secs(2),
    };
    let agent = Agent::new(AgentOptions {
        config_dir: Some(root.join("config")),
        browser_config: Some(cfg),
        clipboard: Some(clip.clone()),
        ..Default::default()
    });
    let mut events = agent.subscribe();
    let metadata = fixture::seed(&agent, &root, opts.fresh, opts.unlocked)
        .map_err(|_| "fixture initialization failed")?;
    // Fixture construction precedes the client's observable session. Keep the
    // receiver, but discard bootstrap notifications before publishing readiness.
    while let Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) =
        events.try_recv()
    {}
    let rpc = std::sync::Mutex::new(latch::rpc::LocalClient::new(&agent, ClientCtx::desktop()));
    let s = Session {
        rpc,
        agent,
        clip,
        root,
        fixture: metadata,
        sync_ready: std::sync::atomic::AtomicBool::new(false),
        sync_setup_attempted: std::sync::atomic::AtomicBool::new(false),
        inspection: std::sync::Mutex::new(json!({"clipboard":null,"lastExport":null})),
    };
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "{}",
        json!({"fixture":s.fixture,"inspection":inspect(&s)})
    )?;
    out.flush()?;
    for line in lines {
        let command: Value = serde_json::from_str(&line?)?;
        let response = match command["operation"].as_str() {
            Some("rpc") => dispatch(&s, serde_json::from_value(command["request"].clone())?),
            Some("inspect") => inspect(&s),
            Some("events") => {
                let mut buffer = Vec::new();
                loop {
                    match events.try_recv() {
                        Ok(event) => buffer.push(event),
                        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
                json!({"events":buffer})
            }
            _ => return Err("unknown worker operation".into()),
        };
        writeln!(out, "{response}")?;
        out.flush()?;
    }
    Ok(())
}
