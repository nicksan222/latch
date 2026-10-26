//! Blocking CLI client over an authenticated tonic Unix-socket connection.
//! A private worker runtime owns the generated async client, so callers may
//! invoke the synchronous API from either ordinary or Tokio threads.

use std::path::PathBuf;
#[cfg(unix)]
use std::sync::Weak;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use latch_rpc::{
    errors::RpcError,
    sdk::{Call, Reply},
    v1::{AgentEvent, ErrorCode, HelloParams, HelloResult},
};

/// Blocking event receiver whose drop cancels the gRPC stream immediately.
pub struct EventSubscription {
    receiver: mpsc::Receiver<Result<AgentEvent, RpcError>>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EventSubscription {
    pub fn recv(&self) -> Result<Result<AgentEvent, RpcError>, mpsc::RecvError> {
        self.receiver.recv()
    }

    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Result<AgentEvent, RpcError>, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

#[cfg(unix)]
enum ConnectionSource {
    Socket,
    Embedded {
        agent: Weak<super::Agent>,
        client: std::os::unix::net::UnixStream,
        server: std::os::unix::net::UnixStream,
    },
}

/// Env override for the binary spawned as headless agent.
pub const ENV_AGENT_BIN: &str = "LATCH_AGENT_BIN";
const SPAWN_WAIT: Duration = Duration::from_secs(15);

#[cfg(unix)]
enum Command {
    Execute {
        call: Box<Call>,
        reply: mpsc::Sender<Result<Reply, RpcError>>,
    },
    WatchEvents {
        reply: mpsc::Sender<Result<EventSubscription, RpcError>>,
    },
    Stop,
}

pub struct AgentClient {
    #[cfg(unix)]
    commands: tokio::sync::mpsc::UnboundedSender<Command>,
    #[cfg(unix)]
    worker: Option<JoinHandle<()>>,
}

fn err(code: ErrorCode, message: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: message.into(),
    }
}

impl AgentClient {
    /// Connect, starting a headless agent only after an ordinary IO failure.
    pub fn connect_or_spawn(hello: HelloParams) -> Result<(Self, HelloResult), RpcError> {
        match Self::connect(hello.clone()) {
            Ok(value) => return Ok(value),
            Err(error) if error.code != ErrorCode::Io => return Err(error),
            Err(_) => {}
        }
        spawn_agent()?;
        let deadline = Instant::now() + SPAWN_WAIT;
        loop {
            match Self::connect(hello.clone()) {
                Ok(value) => return Ok(value),
                Err(error) if error.code != ErrorCode::Io => return Err(error),
                Err(error) => {
                    if Instant::now() >= deadline {
                        return Err(err(
                            ErrorCode::Io,
                            format!("agent did not start: {}", error.message),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    /// Connect only; no process is spawned.
    #[cfg(unix)]
    pub fn connect(hello: HelloParams) -> Result<(Self, HelloResult), RpcError> {
        // Fail closed before launching the worker. The connector repeats these
        // checks on every connection and reconnect before sending HTTP/2 bytes.
        let _ = verified_socket()?;
        Self::connect_with_source(hello, ConnectionSource::Socket)
    }

    /// Generated gRPC transport for a trusted embedded application agent.
    #[cfg(unix)]
    pub fn connect_embedded(
        agent: Weak<super::Agent>,
        hello: HelloParams,
    ) -> Result<(Self, HelloResult), RpcError> {
        let (client, server) = std::os::unix::net::UnixStream::pair()
            .map_err(|_| err(ErrorCode::Io, "cannot create embedded agent channel"))?;
        verify_server(&client)?;
        verify_server(&server)?;
        Self::connect_with_source(
            hello,
            ConnectionSource::Embedded {
                agent,
                client,
                server,
            },
        )
    }

    #[cfg(not(unix))]
    pub fn connect_embedded(
        _agent: std::sync::Weak<super::Agent>,
        _hello: HelloParams,
    ) -> Result<(Self, HelloResult), RpcError> {
        Err(err(
            ErrorCode::Internal,
            "embedded agent transport is not supported on this platform yet",
        ))
    }

    #[cfg(unix)]
    fn connect_with_source(
        hello: HelloParams,
        source: ConnectionSource,
    ) -> Result<(Self, HelloResult), RpcError> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (commands, requests) = tokio::sync::mpsc::unbounded_channel();
        let worker = std::thread::Builder::new()
            .name("latch-agent-client".into())
            .spawn(move || worker_main(hello, source, requests, ready_tx))
            .map_err(|_| err(ErrorCode::Io, "cannot start agent client"))?;
        match ready_rx.recv() {
            Ok(Ok(result)) => Ok((
                Self {
                    commands,
                    worker: Some(worker),
                },
                result,
            )),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(err(ErrorCode::Io, "agent client stopped during connection"))
            }
        }
    }

    #[cfg(not(unix))]
    pub fn connect(_hello: HelloParams) -> Result<(Self, HelloResult), RpcError> {
        Err(err(
            ErrorCode::Internal,
            "the agent socket is not supported on this platform yet",
        ))
    }

    /// Execute a typed request through the generated gRPC client.
    #[cfg(unix)]
    pub fn execute(&mut self, call: Call) -> Result<Reply, RpcError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(Command::Execute {
                call: Box::new(call),
                reply,
            })
            .map_err(|_| err(ErrorCode::Io, "agent connection closed"))?;
        response
            .recv()
            .map_err(|_| err(ErrorCode::Io, "agent connection closed"))?
    }

    /// Receive authenticated server events while unary requests continue.
    #[cfg(unix)]
    pub fn watch_events(&mut self) -> Result<EventSubscription, RpcError> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(Command::WatchEvents { reply })
            .map_err(|_| err(ErrorCode::Io, "agent connection closed"))?;
        response
            .recv()
            .map_err(|_| err(ErrorCode::Io, "agent connection closed"))?
    }

    #[cfg(not(unix))]
    pub fn watch_events(&mut self) -> Result<EventSubscription, RpcError> {
        Err(err(
            ErrorCode::Internal,
            "the agent socket is not supported on this platform yet",
        ))
    }

    #[cfg(not(unix))]
    pub fn execute(&mut self, _call: Call) -> Result<Reply, RpcError> {
        Err(err(
            ErrorCode::Internal,
            "the agent socket is unsupported on this platform",
        ))
    }
}

#[cfg(unix)]
impl Drop for AgentClient {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(unix)]
fn verified_socket() -> Result<std::os::unix::net::UnixStream, RpcError> {
    let dir = crate::core::paths::runtime_dir();
    match crate::core::paths::check_secure_dir(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(err(ErrorCode::Io, "agent not running"));
        }
        Err(_) => return Err(err(ErrorCode::Denied, "insecure agent runtime dir")),
    }
    let path = dir.join(super::SOCKET_FILE);
    let stream = std::os::unix::net::UnixStream::connect(path).map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::PermissionDenied {
            ErrorCode::Denied
        } else {
            ErrorCode::Io
        };
        err(code, "cannot connect to agent")
    })?;
    verify_server(&stream)?;
    Ok(stream)
}

#[cfg(unix)]
fn worker_main(
    hello: HelloParams,
    source: ConnectionSource,
    mut commands: tokio::sync::mpsc::UnboundedReceiver<Command>,
    ready: mpsc::SyncSender<Result<HelloResult, RpcError>>,
) {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tonic::transport::Endpoint;
    use tower::service_fn;

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            let _ = ready.send(Err(err(ErrorCode::Io, "cannot start agent client runtime")));
            return;
        }
    };
    let denied = Arc::new(AtomicBool::new(false));
    let connector_denied = denied.clone();
    let connected = runtime.block_on(async {
        let (one_shot, server_task) = match source {
            ConnectionSource::Socket => (None, None),
            ConnectionSource::Embedded {
                agent,
                client,
                server,
            } => {
                let one_shot = Arc::new(Mutex::new(Some(client)));
                server
                    .set_nonblocking(true)
                    .map_err(|_| err(ErrorCode::Io, "cannot start embedded agent channel"))?;
                let server = tokio::net::UnixStream::from_std(server)
                    .map_err(|_| err(ErrorCode::Io, "cannot start embedded agent channel"))?;
                let task = tokio::spawn(super::server::serve_embedded(agent, server));
                (Some(one_shot), Some(task))
            }
        };
        let endpoint = Endpoint::from_static("http://[::]:50051")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30));
        let channel = endpoint
            .connect_with_connector(service_fn(move |_| {
                let denied = connector_denied.clone();
                let one_shot = one_shot.clone();
                async move {
                    let connection = match one_shot {
                        Some(slot) => match slot.lock().unwrap().take() {
                            Some(stream) => verify_server(&stream).map(|_| stream),
                            None => Err(err(ErrorCode::Io, "embedded agent connection closed")),
                        },
                        None => verified_socket(),
                    };
                    let stream = match connection {
                        Ok(stream) => stream,
                        Err(error) => {
                            if error.code == ErrorCode::Denied {
                                denied.store(true, Ordering::SeqCst);
                            }
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                error.message,
                            ));
                        }
                    };
                    stream.set_nonblocking(true)?;
                    let stream = tokio::net::UnixStream::from_std(stream)?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await
            .map_err(|_| {
                let code = if denied.load(Ordering::SeqCst) {
                    ErrorCode::Denied
                } else {
                    ErrorCode::Io
                };
                err(code, "cannot establish agent gRPC connection")
            })?;
        let mut client = latch_rpc::v1::latch_agent_client::LatchAgentClient::new(channel)
            .max_decoding_message_size(16 * 1024 * 1024)
            .max_encoding_message_size(16 * 1024 * 1024);
        let result = client
            .hello(latch_rpc::client::request(hello.clone(), &hello)?)
            .await
            .map_err(|error| {
                if denied.load(Ordering::SeqCst) {
                    err(ErrorCode::Denied, "insecure agent connection")
                } else {
                    latch_rpc::errors::rpc_error(error)
                }
            })?
            .into_inner();
        Ok::<_, RpcError>((client, result, server_task))
    });
    let (mut client, result, server_task) = match connected {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(result)).is_err() {
        return;
    }
    runtime.block_on(async {
        let mut event_task: Option<tokio::task::JoinHandle<()>> = None;
        while let Some(command) = commands.recv().await {
            match command {
                Command::Stop => break,
                Command::Execute { call, reply } => {
                    let result = latch_rpc::client::execute(&mut client, *call, &hello).await;
                    let result = if denied.load(Ordering::SeqCst) {
                        Err(err(ErrorCode::Denied, "insecure agent connection"))
                    } else {
                        result
                    };
                    let _ = reply.send(result);
                }
                Command::WatchEvents { reply } => {
                    if let Some(task) = event_task.take() {
                        task.abort();
                    }
                    let request = latch_rpc::client::request(latch_rpc::v1::Empty {}, &hello);
                    let result = match request {
                        Ok(request) => client.watch_events(request).await.map(|response| response.into_inner())
                            .map_err(latch_rpc::errors::rpc_error),
                        Err(error) => Err(error),
                    };
                    match result {
                        Ok(mut stream) => {
                            let (events_tx, events_rx) = mpsc::channel();
                            let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
                            event_task = Some(tokio::spawn(async move {
                                loop {
                                    tokio::select! {
                                        _ = &mut cancelled => break,
                                        event = stream.message() => {
                                            let event = match event {
                                                Ok(Some(message)) => Ok(message),
                                                Ok(None) => break,
                                                Err(error) => Err(latch_rpc::errors::rpc_error(error)),
                                            };
                                            let failed = event.is_err();
                                            if events_tx.send(event).is_err() || failed { break; }
                                        }
                                    }
                                }
                            }));
                            let _ = reply.send(Ok(EventSubscription { receiver: events_rx, cancel: Some(cancel) }));
                        }
                        Err(error) => { let _ = reply.send(Err(error)); }
                    }
                }
            }
        }
        if let Some(task) = event_task {
            task.abort();
        }
    });
    if let Some(task) = server_task {
        task.abort();
    }
    runtime.shutdown_timeout(Duration::from_millis(100));
}

/// The socket's peer must run as our UID: another user's process bound to
/// our socket path would otherwise receive passphrases and secrets.
#[cfg(unix)]
pub fn verify_server(stream: &std::os::unix::net::UnixStream) -> Result<(), RpcError> {
    let me = crate::core::paths::uid();
    match super::peer_uid(stream) {
        Ok(uid) if uid == me => Ok(()),
        Ok(_) => Err(err(
            ErrorCode::Denied,
            "the agent socket is served by another user; refusing to connect",
        )),
        Err(e) => Err(err(
            ErrorCode::Denied,
            format!("cannot verify the agent socket owner: {e}"),
        )),
    }
}

/// The `latch` binary used to spawn a headless agent (see module docs).
/// Never falls back to `PATH`.
pub fn agent_binary() -> Result<PathBuf, RpcError> {
    if let Some(p) = std::env::var_os(ENV_AGENT_BIN).filter(|v| !v.is_empty()) {
        let p = PathBuf::from(p);
        if !p.is_absolute() || !p.is_file() {
            return Err(err(
                ErrorCode::InvalidParams,
                format!("{ENV_AGENT_BIN} must be an absolute path to the latch binary"),
            ));
        }
        return Ok(p);
    }
    let exe_name = format!("latch{}", std::env::consts::EXE_SUFFIX);
    if let Ok(exe) = std::env::current_exe() {
        if exe.file_name().is_some_and(|n| n == exe_name.as_str()) {
            return Ok(exe);
        }
        if let Some(dir) = exe.parent() {
            let sib = dir.join(&exe_name);
            if sib.is_file() {
                return Ok(sib);
            }
        }
    }
    Err(err(
        ErrorCode::NotFound,
        format!(
            "cannot find the latch binary next to this executable; start the agent manually or set {ENV_AGENT_BIN} to its absolute path"
        ),
    ))
}

fn spawn_agent() -> Result<(), RpcError> {
    use std::process::{Command, Stdio};
    let bin = agent_binary()?;
    let mut cmd = Command::new(&bin);
    cmd.args(["agent", "run", "--headless"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own session: survives the parent's terminal closing / Ctrl-C.
        // SAFETY: setsid is async-signal-safe and touches no Rust state.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    let child = cmd.spawn().map_err(|e| {
        err(
            ErrorCode::Io,
            format!("cannot start agent ({}): {e}", bin.display()),
        )
    })?;
    // Reap in the background so no zombie lingers while we run.
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn same_user_server_is_accepted() {
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        verify_server(&a).unwrap();
    }
}
