//! Private per-session agent worker. IPC is JSON lines over inherited private pipes.
use crate::{Options, fixture};
use latch::agent::{
    Agent, AgentOptions, ClientCtx,
    browser::ManagerConfig,
    clipboard::{ClipboardBackend, MemoryClipboard},
};
use latch_rpc::{
    RpcError,
    sdk::{Call, Reply},
    v1::{self, ErrorCode},
};
use prost::Message;
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
fn denied() -> RpcError {
    RpcError {
        code: ErrorCode::Denied,
        message: "operation unavailable outside fixture arena".into(),
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
fn adapt(s: &Session, call: &mut Call) -> Result<(), RpcError> {
    use Call::*;
    match call {
        VaultClone(_) | SyncSetRemote(_) | VaultLocate(_) | DoctorScan(_) | HooksInstall(_)
        | HooksUninstall(_) => return Err(denied()),
        SyncRun(_) | SyncResolve(_)
            if !s.sync_ready.load(std::sync::atomic::Ordering::SeqCst)
                || s.agent.status().path.as_deref() != s.fixture["vaults"]["personal"].as_str() =>
        {
            return Err(denied());
        }
        RecipientsAdd(p) => {
            let key = &p.public_key;
            if !key.starts_with("age1")
                || key.len() != 62
                || key[4..].contains('1')
                || latch::core::crypto::parse_recipient(key).is_err()
            {
                return Err(denied());
            }
        }
        VaultUnlock(p) if p.identity_file.is_some() => return Err(denied()),
        ImportPreview(p) | ImportApply(p) if p.format == v1::ImportFormat::Pass as i32 => {
            return Err(denied());
        }
        VaultBenchmarkKdf(p) if p.target_ms > 1000 => return Err(denied()),
        _ => {}
    }
    let vault_path = matches!(call, VaultInit(_) | VaultOpen(_));
    let path = match call {
        VaultInit(p) => {
            if p.kdf.as_ref().is_some_and(|k| {
                !(65536..=262144).contains(&k.memory_kib)
                    || !(1..=5).contains(&k.iterations)
                    || !(1..=4).contains(&k.parallelism)
            }) {
                return Err(denied());
            }
            p.remote_url = None;
            p.git_name = Some("Fixture".into());
            p.git_email = Some("fixture@example.invalid".into());
            Some(&mut p.path)
        }
        VaultOpen(p) => Some(&mut p.path),
        ImportPreview(p) | ImportApply(p) => Some(&mut p.path),
        ExportRun(p) => Some(&mut p.path),
        _ => None,
    };
    if let Some(raw) = path {
        let path = if vault_path
            && s.fixture["vaults"]
                .as_object()
                .is_some_and(|v| v.values().any(|v| v.as_str() == Some(raw)))
        {
            PathBuf::from(&*raw)
        } else {
            safe_path(s, raw).ok_or_else(denied)?
        };
        std::fs::create_dir_all(path.parent().ok_or_else(denied)?).map_err(|_| denied())?;
        *raw = path.to_string_lossy().into_owned();
    }
    Ok(())
}
fn dispatch(s: &Session, method: &str, payload: &[u8]) -> Value {
    let result = (|| {
        let mut call = Call::decode(method, payload)?;
        let export = match &call {
            Call::ExportRun(p) => Some((p.path.clone(), p.format)),
            _ => None,
        };
        adapt(s, &mut call)?;
        let reply = if matches!(call, Call::BrowserDetect(_)) {
            Reply::BrowserDetect(v1::BrowserDetectResponse { items: vec![] })
        } else {
            s.rpc.lock().unwrap().execute(call)?
        };
        if let (Some((path, format)), Reply::ExportRun(result)) = (export, &reply) {
            let content = std::fs::read(&result.path)
                .ok()
                .filter(|b| b.len() <= 2 * 1024 * 1024)
                .and_then(|b| String::from_utf8(b).ok());
            s.inspection.lock().unwrap()["lastExport"] =
                json!({"path":path,"format":format,"content":content});
        }
        Ok::<_, RpcError>(reply.encode())
    })();
    match result {
        Ok(bytes) => json!({"bytes":bytes}),
        Err(error) => json!({"error":{"code":error.code as i32,"message":error.message}}),
    }
}
fn sync_scenario(s: &Session, scenario: &str) -> Value {
    if !matches!(scenario, "conflict" | "remote_change")
        || !s.agent.is_unlocked()
        || s.agent.status().path.as_deref() != s.fixture["vaults"]["personal"].as_str()
        || s.sync_setup_attempted
            .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return json!({"error":{"code":ErrorCode::Denied as i32,"message":denied().message}});
    }
    match fixture::sync_scenario(&s.agent, &s.root, &s.fixture, scenario == "conflict") {
        Ok(result) => {
            s.sync_ready
                .store(true, std::sync::atomic::Ordering::SeqCst);
            json!({"result":result})
        }
        Err(()) => json!({"error":{"code":ErrorCode::Denied as i32,"message":denied().message}}),
    }
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
            Some("rpc") => dispatch(
                &s,
                command["method"].as_str().ok_or("missing method")?,
                &serde_json::from_value::<Vec<u8>>(command["payload"].clone())?,
            ),
            Some("sync-scenario") => {
                sync_scenario(&s, command["scenario"].as_str().ok_or("missing scenario")?)
            }
            Some("inspect") => inspect(&s),
            Some("events") => {
                let mut buffer = Vec::new();
                loop {
                    match events.try_recv() {
                        Ok(event) => buffer.push(v1::AgentEvent::from(event).encode_to_vec()),
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
