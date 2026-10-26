//! Development-only HTTP adapter; each capability owns an isolated core-agent process.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use latch::protocol::RpcRequest;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
mod fixture;
mod worker;
#[derive(Clone)]
struct App {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    origins: Arc<Vec<String>>,
    root: PathBuf,
    capacity: Arc<tokio::sync::Semaphore>,
    idle: Duration,
    allocation_lease: Duration,
}
struct Session {
    resources: Mutex<Option<Resources>>,
    origin: String,
    activity: Mutex<Activity>,
    closed: tokio::sync::watch::Sender<bool>,
    polling: tokio::sync::Mutex<()>,
    latency: u64,
    kdf_delay: u64,
}
struct Activity {
    touched: Instant,
    activated: bool,
}
struct Resources {
    worker: Worker,
    _arena: tempfile::TempDir,
    _capacity: tokio::sync::OwnedSemaphorePermit,
}
struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    output: std::io::BufReader<ChildStdout>,
}
impl Worker {
    fn call(&mut self, input: Value) -> Result<Value, StatusCode> {
        let pipe = self.input.as_mut().ok_or(StatusCode::GONE)?;
        writeln!(pipe, "{input}")
            .and_then(|_| pipe.flush())
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
        self.read()
    }
    fn read(&mut self) -> Result<Value, StatusCode> {
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
        serde_json::from_str(&line).map_err(|_| StatusCode::BAD_GATEWAY)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        drop(self.input.take());
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    #[serde(default)]
    fresh: bool,
    #[serde(default)]
    unlocked: bool,
    #[serde(default)]
    latency_ms: u64,
    #[serde(default)]
    kdf_delay_ms: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Call {
    session_id: String,
    request: RpcRequest,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Capability {
    session_id: String,
}
type HttpError = StatusCode;
fn origin(h: &HeaderMap) -> Result<&str, HttpError> {
    h.get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)
}
fn session(app: &App, h: &HeaderMap, id: &str, activate: bool) -> Result<Arc<Session>, HttpError> {
    let sessions = app.sessions.lock().unwrap();
    let s = sessions
        .get(id)
        .filter(|s| origin(h).ok() == Some(s.origin.as_str()))
        .ok_or(StatusCode::FORBIDDEN)?
        .clone();
    if activate {
        let mut activity = s.activity.lock().unwrap();
        activity.touched = Instant::now();
        activity.activated = true;
    }
    Ok(s)
}
async fn guard(
    State(app): State<App>,
    req: axum::extract::Request,
    next: Next,
) -> Result<Response, HttpError> {
    if !matches!(
        req.uri().path(),
        "/__latch/health"
            | "/__latch/session"
            | "/__latch/rpc"
            | "/__latch/events"
            | "/__latch/inspect"
    ) {
        return Err(StatusCode::NOT_FOUND);
    }
    if req.uri().path() == "/__latch/health" {
        return Ok(next.run(req).await);
    }
    if !origin(req.headers()).is_ok_and(|o| app.origins.iter().any(|x| x == o)) {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut r = next.run(req).await;
    if r.status().is_client_error() || r.status().is_server_error() {
        r = r.status().into_response();
    }
    r.headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    r.headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    Ok(r)
}
fn configure(command: &mut Command, root: &std::path::Path) {
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_0", root.join("hooks"))
        .env("GIT_CONFIG_KEY_1", "init.templateDir")
        .env("GIT_CONFIG_VALUE_1", root.join("templates"));
    for (key, dir) in [
        ("LATCH_CONFIG_DIR", "config"),
        ("LATCH_DATA_DIR", "data"),
        ("LATCH_RUNTIME_DIR", "runtime"),
        ("LATCH_RAM_DIR", "ram"),
    ] {
        command.env(key, root.join(dir));
    }
}
async fn create(
    State(app): State<App>,
    h: HeaderMap,
    Json(opts): Json<Options>,
) -> Result<Json<Value>, HttpError> {
    if opts.latency_ms > 10_000 || opts.kdf_delay_ms > 10_000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let o = origin(&h)?.to_owned();
    let permit = app
        .capacity
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    tokio::task::spawn_blocking(move || {
        let arena =
            tempfile::tempdir_in(&app.root).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut command =
            Command::new(std::env::current_exe().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?);
        configure(&mut command, arena.path());
        command
            .arg("--worker")
            .arg(arena.path())
            .current_dir(arena.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = command
            .spawn()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let input = child.stdin.take();
        let output = std::io::BufReader::new(
            child
                .stdout
                .take()
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        );
        let mut worker = Worker {
            child,
            input,
            output,
        };
        let pipe = worker
            .input
            .as_mut()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        writeln!(
            pipe,
            "{}",
            serde_json::to_string(&opts).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        )
        .and_then(|_| pipe.flush())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut metadata = worker.read()?;
        let mut random = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut random);
        let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
        metadata["sessionId"] = json!(id);
        let session = Arc::new(Session {
            resources: Mutex::new(Some(Resources {
                worker,
                _arena: arena,
                _capacity: permit,
            })),
            origin: o,
            activity: Mutex::new(Activity {
                touched: Instant::now(),
                activated: false,
            }),
            closed: tokio::sync::watch::channel(false).0,
            polling: tokio::sync::Mutex::new(()),
            latency: opts.latency_ms,
            kdf_delay: opts.kdf_delay_ms,
        });
        app.sessions.lock().unwrap().insert(id, session);
        Ok(Json(metadata))
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}
async fn invoke(s: Arc<Session>, command: Value) -> Result<Value, HttpError> {
    tokio::task::spawn_blocking(move || {
        let mut resources = s.resources.lock().unwrap();
        resources
            .as_mut()
            .ok_or(StatusCode::GONE)?
            .worker
            .call(command)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}
async fn rpc(
    State(app): State<App>,
    h: HeaderMap,
    Json(c): Json<Call>,
) -> Result<Json<Value>, HttpError> {
    let s = session(&app, &h, &c.session_id, true)?;
    let delay = s.latency
        + if matches!(
            c.request.method.as_str(),
            "vault.unlock" | "vault.init" | "vault.benchmarkKdf"
        ) {
            s.kdf_delay
        } else {
            0
        };
    let mut closed = s.closed.subscribe();
    if *closed.borrow() {
        return Err(StatusCode::GONE);
    }
    tokio::select! {
        _ = closed.changed() => return Err(StatusCode::GONE),
        _ = tokio::time::sleep(Duration::from_millis(delay)) => {}
    }
    Ok(Json(
        invoke(s, json!({"operation":"rpc","request":c.request})).await?,
    ))
}
async fn inspection(
    State(app): State<App>,
    h: HeaderMap,
    Json(p): Json<Capability>,
) -> Result<Json<Value>, HttpError> {
    Ok(Json(
        invoke(
            session(&app, &h, &p.session_id, true)?,
            json!({"operation":"inspect"}),
        )
        .await?,
    ))
}
async fn events(
    State(app): State<App>,
    h: HeaderMap,
    Json(p): Json<Capability>,
) -> Result<Json<Value>, HttpError> {
    let s = session(&app, &h, &p.session_id, true)?;
    let _poll = s.polling.try_lock().map_err(|_| StatusCode::CONFLICT)?;
    let mut closed = s.closed.subscribe();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if *closed.borrow() {
            return Err(StatusCode::GONE);
        }
        let out = invoke(s.clone(), json!({"operation":"events"})).await?;
        if out["events"].as_array().is_some_and(|v| !v.is_empty()) || Instant::now() >= until {
            return Ok(Json(out));
        }
        tokio::select! {
            _ = closed.changed() => return Err(StatusCode::GONE),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}
async fn remove(
    State(app): State<App>,
    h: HeaderMap,
    Json(p): Json<Capability>,
) -> Result<StatusCode, HttpError> {
    let s = session(&app, &h, &p.session_id, false)?;
    app.sessions.lock().unwrap().remove(&p.session_id);
    close_sessions(vec![s]).await;
    Ok(StatusCode::NO_CONTENT)
}
// Resource ownership is independent of request Arcs: a closing poll or delayed
// RPC cannot retain the worker, arena, or capacity after deletion.
async fn close_sessions(sessions: Vec<Arc<Session>>) {
    for session in &sessions {
        session.closed.send_replace(true);
    }
    let cleanup: Vec<_> = sessions
        .into_iter()
        .map(|session| {
            tokio::task::spawn_blocking(move || {
                let resources = session.resources.lock().unwrap().take();
                drop(resources);
            })
        })
        .collect();
    for task in cleanup {
        let _ = task.await;
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os()
        .next()
        .as_deref()
        .map(std::path::Path::new)
        .and_then(std::path::Path::file_name)
        .is_some_and(|n| n == "fixture-browser")
    {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }
    let mut args = std::env::args().skip(1);
    let first = args.next();
    if first.as_deref() == Some("--worker") {
        return worker::run(PathBuf::from(args.next().ok_or("missing private arena")?));
    }
    let mut port = 1430;
    let mut idle_secs = 600;
    let mut allocation_lease_secs = 15;
    let mut origins = vec![
        "http://localhost:1420".into(),
        "http://127.0.0.1:1420".into(),
        "http://localhost:1423".into(),
        "http://127.0.0.1:1423".into(),
    ];
    let mut args = first.into_iter().chain(args);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().ok_or("missing port")?.parse()?,
            "--idle-secs" => {
                idle_secs = args.next().ok_or("missing idle duration")?.parse::<u64>()?;
                if idle_secs == 0 {
                    return Err("idle duration must be positive".into());
                }
            }
            "--allocation-lease-secs" => {
                allocation_lease_secs = args
                    .next()
                    .ok_or("missing allocation lease")?
                    .parse::<u64>()?;
                if allocation_lease_secs == 0 {
                    return Err("allocation lease must be positive".into());
                }
            }
            "--origin" => {
                let o = args.next().ok_or("missing origin")?;
                if !(o.starts_with("http://localhost:") || o.starts_with("http://127.0.0.1:"))
                    || o.rsplit(':').next().unwrap().parse::<u16>().is_err()
                {
                    return Err("origin must be a loopback HTTP origin".into());
                }
                origins.push(o);
            }
            _ => return Err("unknown bridge argument".into()),
        }
    }
    let root = tempfile::tempdir()?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let app = App {
            sessions: Default::default(),
            origins: Arc::new(origins),
            root: root.path().to_owned(),
            capacity: Arc::new(tokio::sync::Semaphore::new(32)),
            idle: Duration::from_secs(idle_secs),
            allocation_lease: Duration::from_secs(allocation_lease_secs),
        };
        let router = Router::new()
            .route("/__latch/health", get(|| async { StatusCode::NO_CONTENT }))
            .route("/__latch/session", post(create).delete(remove))
            .route("/__latch/rpc", post(rpc))
            .route("/__latch/events", post(events))
            .route("/__latch/inspect", post(inspection))
            .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
            .layer(middleware::from_fn_with_state(app.clone(), guard))
            .with_state(app.clone());
        let cleanup = app.clone();
        let janitor = tokio::spawn(async move {
            loop {
                tokio::time::sleep(
                    cleanup
                        .idle
                        .min(cleanup.allocation_lease)
                        .min(Duration::from_secs(1)),
                )
                .await;
                let old = {
                    let mut sessions = cleanup.sessions.lock().unwrap();
                    let ids: Vec<_> = sessions
                        .iter()
                        .filter(|(_, s)| {
                            let activity = s.activity.lock().unwrap();
                            activity.touched.elapsed()
                                >= if activity.activated {
                                    cleanup.idle
                                } else {
                                    cleanup.allocation_lease
                                }
                        })
                        .map(|(id, _)| id.clone())
                        .collect();
                    ids.into_iter()
                        .filter_map(|id| sessions.remove(&id))
                        .collect::<Vec<_>>()
                };
                close_sessions(old).await;
            }
        });
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        eprintln!(
            "Latch fixture bridge listening on {}",
            listener.local_addr()?
        );
        let shutdown = app.clone();
        let result = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("signal handler");
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
                let sessions = std::mem::take(&mut *shutdown.sessions.lock().unwrap());
                close_sessions(sessions.into_values().collect()).await;
            })
            .await;
        janitor.abort();
        let sessions = std::mem::take(&mut *app.sessions.lock().unwrap());
        close_sessions(sessions.into_values().collect()).await;
        result
    })?;
    Ok(())
}
