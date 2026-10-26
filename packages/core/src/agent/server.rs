//! Authenticated gRPC over the private agent Unix socket.
//!
//! - `paths::runtime_dir()` is created 0700 (refused when it is a symlink or
//!   owned by another user); `agent.sock` is chmod 0600.
//! - Single instance: an exclusive `fs2` lock on `agent.lock` beside the
//!   socket. Holding it proves any existing socket file is stale, so it is
//!   removed before binding.
//! - Every accepted connection's peer UID must equal ours (`SO_PEERCRED` on
//!   Linux, `getpeereid` elsewhere); others are dropped immediately.
//! - Every gRPC request carries client identity metadata; accepted sockets
//!   carry verified peer credentials into tonic request extensions.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll};
use std::thread::JoinHandle;

use fs2::FileExt;
use latch_rpc::v1::latch_agent_server::LatchAgentServer;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::watch;
use tokio_stream::{StreamExt, wrappers::UnixListenerStream};
use tonic::transport::{Server, server::Connected};

use super::{Agent, ClientCtx};
use crate::core::{Error, Result, paths};
use crate::protocol::ClientKind;

#[derive(Clone, Debug)]
pub(crate) struct PeerInfo {
    uid: u32,
}

/// Authenticate the Unix peer and claimed application identity for each RPC.
pub(crate) fn client_context<T>(
    request: &tonic::Request<T>,
) -> std::result::Result<ClientCtx, tonic::Status> {
    let peer = request
        .extensions()
        .get::<PeerInfo>()
        .ok_or_else(|| tonic::Status::permission_denied("missing peer credentials"))?;
    if peer.uid != paths::uid() {
        return Err(tonic::Status::permission_denied("foreign peer"));
    }
    let kind = match request
        .metadata()
        .get("x-latch-client")
        .and_then(|v| v.to_str().ok())
    {
        Some("cli") => ClientKind::Cli,
        Some("desktop") => ClientKind::Desktop,
        _ => return Err(tonic::Status::permission_denied("invalid client kind")),
    };
    if request
        .metadata()
        .get("x-latch-version")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .is_none()
    {
        return Err(tonic::Status::permission_denied("missing client version"));
    }
    Ok(ClientCtx { kind })
}

pub(crate) struct ServerState {
    path: PathBuf,
    pid_path: PathBuf,
    lock_file: File,
    shutdown: watch::Sender<bool>,
    thread: Option<JoinHandle<()>>,
}

/// UID of the process on the other end of a Unix socket.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn peer_uid(s: &UnixStream) -> io::Result<u32> {
    peer_uid_fd(s.as_raw_fd())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn peer_uid_fd(fd: RawFd) -> io::Result<u32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: valid fd, pointer and length for SO_PEERCRED.
    let r = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut len,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(cred.uid)
}

/// UID of the process on the other end of a Unix socket.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn peer_uid(s: &UnixStream) -> io::Result<u32> {
    peer_uid_fd(s.as_raw_fd())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(crate) fn peer_uid_fd(fd: RawFd) -> io::Result<u32> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: valid fd and out-pointers.
    let r = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

fn io_err(ctx: impl Into<String>) -> impl FnOnce(io::Error) -> Error {
    let ctx = ctx.into();
    move |e| Error::Io {
        context: ctx,
        source: e,
    }
}

struct CountedStream {
    stream: tokio::net::UnixStream,
    peer: PeerInfo,
    agent: Weak<Agent>,
}

impl Drop for CountedStream {
    fn drop(&mut self) {
        if let Some(agent) = self.agent.upgrade() {
            agent.connection_closed();
        }
    }
}

impl Connected for CountedStream {
    type ConnectInfo = PeerInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        self.peer.clone()
    }
}

impl AsyncRead for CountedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for CountedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl ServerState {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn start(agent: Weak<Agent>) -> Result<ServerState> {
        let dir = paths::runtime_dir();
        // Refuses symlinked or foreign dirs (a pre-created runtime dir would
        // let another user replace the socket).
        paths::ensure_secure_dir(&dir).map_err(io_err(format!("create {}", dir.display())))?;
        let lock_path = dir.join(super::LOCK_FILE);
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(io_err(format!("open {}", lock_path.display())))?;
        if lock_file.try_lock_exclusive().is_err() {
            return Err(Error::AlreadyExists("a running Latch agent".into()));
        }
        let path = dir.join(super::SOCKET_FILE);
        if path.exists() || std::fs::symlink_metadata(&path).is_ok() {
            // We hold the lock: whatever is there is stale.
            std::fs::remove_file(&path)
                .map_err(io_err(format!("remove stale {}", path.display())))?;
        }
        let listener =
            UnixListener::bind(&path).map_err(io_err(format!("bind {}", path.display())))?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(io_err(format!("chmod {}", path.display())))?;
        }
        listener
            .set_nonblocking(true)
            .map_err(io_err("configure agent socket"))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(io_err("start agent gRPC runtime"))?;
        let pid_path = dir.join(super::PID_FILE);
        let _ = std::fs::write(&pid_path, format!("{}\n", std::process::id()));
        let (shutdown, mut receiver) = watch::channel(false);
        let thread = std::thread::Builder::new()
            .name("latch-agent-grpc".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let listener = match tokio::net::UnixListener::from_std(listener) {
                        Ok(listener) => listener,
                        Err(error) => {
                            tracing::error!(%error, "cannot start agent gRPC listener");
                            return;
                        }
                    };
                    let service = LatchAgentServer::new(crate::rpc::GrpcService::new(agent.clone()))
                        .max_decoding_message_size(16 * 1024 * 1024)
                        .max_encoding_message_size(16 * 1024 * 1024);
                    let incoming = UnixListenerStream::new(listener).filter_map(move |result| {
                        let stream = match result {
                            Ok(stream) => stream,
                            Err(error) => return Some(Err(error)),
                        };
                        let uid = match peer_uid_fd(stream.as_raw_fd()) {
                            Ok(uid) if uid == paths::uid() => uid,
                            _ => {
                                tracing::warn!("rejected agent connection from another user");
                                return None;
                            }
                        };
                        let owner = agent.upgrade()?;
                        if owner.is_shutting_down() {
                            return None;
                        }
                        owner.connection_opened();
                        drop(owner);
                        Some(Ok(CountedStream {
                            stream,
                            peer: PeerInfo { uid },
                            agent: agent.clone(),
                        }))
                    });
                    // Dropping the serve future forces active HTTP/2 connections
                    // closed; graceful shutdown can wait forever on idle clients.
                    tokio::select! {
                        result = Server::builder().add_service(service).serve_with_incoming(incoming) => {
                            if let Err(error) = result {
                                tracing::error!(%error, "agent gRPC server failed");
                            }
                        }
                        _ = receiver.changed() => {}
                    }
                });
                // A domain call may still be on Tokio's blocking pool. Keep
                // socket shutdown bounded; generation checks guard late work.
                runtime.shutdown_timeout(std::time::Duration::from_millis(100));
            })
            .map_err(io_err("spawn agent gRPC thread"))?;
        Ok(ServerState {
            path,
            pid_path,
            lock_file,
            shutdown,
            thread: Some(thread),
        })
    }

    pub(crate) fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        if self.thread.is_none() {
            return;
        }
        let _ = self.shutdown.send(true);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.pid_path);
        let _ = FileExt::unlock(&self.lock_file);
    }
}

impl Drop for ServerState {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// Serve one private socket pair for an embedded application client. The
/// caller owns the runtime task and aborts it when its client worker exits.
pub(crate) async fn serve_embedded(agent: Weak<Agent>, stream: tokio::net::UnixStream) {
    let uid = match peer_uid_fd(stream.as_raw_fd()) {
        Ok(uid) if uid == paths::uid() => uid,
        _ => return,
    };
    let Some(owner) = agent.upgrade() else { return };
    owner.connection_opened();
    drop(owner);
    let incoming = tokio_stream::once(Ok::<_, io::Error>(CountedStream {
        stream,
        peer: PeerInfo { uid },
        agent: agent.clone(),
    }));
    let service = LatchAgentServer::new(crate::rpc::GrpcService::new(agent))
        .max_decoding_message_size(16 * 1024 * 1024)
        .max_encoding_message_size(16 * 1024 * 1024);
    if let Err(error) = Server::builder()
        .add_service(service)
        .serve_with_incoming(incoming)
        .await
    {
        tracing::error!(%error, "embedded agent gRPC server failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_request_needs_same_user_peer_and_client_metadata() {
        let mut request = tonic::Request::new(());
        assert!(client_context(&request).is_err());
        request
            .extensions_mut()
            .insert(PeerInfo { uid: paths::uid() });
        assert!(client_context(&request).is_err());
        request
            .metadata_mut()
            .insert("x-latch-client", "cli".parse().unwrap());
        request
            .metadata_mut()
            .insert("x-latch-version", "test".parse().unwrap());
        assert_eq!(client_context(&request).unwrap().kind, ClientKind::Cli);
        request.extensions_mut().insert(PeerInfo {
            uid: paths::uid().wrapping_add(1),
        });
        assert!(client_context(&request).is_err());
    }
}
