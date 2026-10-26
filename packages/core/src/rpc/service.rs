//! The generated gRPC service invokes typed core procedures directly.
use crate::agent::Agent;
use latch_rpc::{
    errors,
    sdk::{Call, Reply},
    v1,
};
use std::pin::Pin;
use std::sync::Weak;
use tonic::{Request, Response, Status};

pub(crate) struct GrpcService {
    agent: Weak<Agent>,
}
impl GrpcService {
    pub(crate) fn new(agent: Weak<Agent>) -> Self {
        Self { agent }
    }
    async fn execute<I>(
        &self,
        request: Request<I>,
        call: impl FnOnce(I) -> Call,
    ) -> Result<Reply, Status> {
        let context = crate::agent::server::client_context(&request)?;
        let agent = self
            .agent
            .upgrade()
            .ok_or_else(|| Status::unavailable("agent stopped"))?;
        let call = call(request.into_inner());
        tokio::task::spawn_blocking(move || agent.execute(&context, call))
            .await
            .map_err(|_| Status::internal("RPC task failed"))?
            .map_err(|error| errors::status(&error))
    }
}
macro_rules! implement_service {
    ($(($method:ident, $variant:ident, $input:ident, $output:ident);)*) => {
        #[tonic::async_trait]
        impl v1::latch_agent_server::LatchAgent for GrpcService {
            $(async fn $method(&self, request: Request<v1::$input>) -> Result<Response<v1::$output>, Status> {
                match self.execute(request, Call::$variant).await? {
                    Reply::$variant(value) => Ok(Response::new(value)),
                    _ => Err(Status::internal("RPC reply mismatch")),
                }
            })*
            type WatchEventsStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<v1::AgentEvent, Status>> + Send>>;
            async fn watch_events(&self, request: Request<v1::Empty>) -> Result<Response<Self::WatchEventsStream>, Status> {
                crate::agent::server::client_context(&request)?;
                let agent = self.agent.upgrade().ok_or_else(|| Status::unavailable("agent stopped"))?;
                let mut events = agent.subscribe();
                let stream = async_stream::try_stream! {
                    loop {
                        match events.recv().await {
                            Ok(event) => {
                                yield event.into();
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => Err(Status::resource_exhausted("event subscriber fell behind"))?,
                        }
                    }
                };
                Ok(Response::new(Box::pin(stream)))
            }
        }
    };
}
latch_rpc::for_each_rpc!(implement_service);
