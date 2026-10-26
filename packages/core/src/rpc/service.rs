//! The generated gRPC server interface is the only external procedure surface.
use std::pin::Pin;
use std::sync::Weak;

use latch_rpc::{codec, v1};
use prost::Message;
use tonic::{Request, Response, Status};

use crate::agent::Agent;
use crate::protocol::RpcRequest;

pub(crate) struct GrpcService {
    agent: Weak<Agent>,
}

impl GrpcService {
    pub(crate) fn new(agent: Weak<Agent>) -> Self {
        Self { agent }
    }

    async fn execute<I, O>(
        &self,
        request: Request<I>,
        method: &'static str,
        input_name: &'static str,
        output_name: &'static str,
    ) -> Result<Response<O>, Status>
    where
        I: Message + Send + 'static,
        O: Message + Default,
    {
        let context = crate::agent::server::client_context(&request)?;
        let params =
            codec::to_legacy(request.get_ref(), input_name).map_err(|e| codec::status(&e))?;
        let agent = self
            .agent
            .upgrade()
            .ok_or_else(|| Status::unavailable("agent stopped"))?;
        let response = tokio::task::spawn_blocking(move || {
            agent.dispatch(
                &context,
                RpcRequest {
                    id: 0,
                    method: method.into(),
                    params,
                },
            )
        })
        .await
        .map_err(|_| Status::internal("RPC task failed"))?;
        if let Some(error) = response.error {
            return Err(codec::status(&error));
        }
        let message = codec::from_legacy(
            response.result.unwrap_or(serde_json::Value::Null),
            output_name,
        )
        .map_err(|e| codec::status(&e))?;
        Ok(Response::new(message))
    }
}

macro_rules! implement_service {
    ($(($method:ident, $alias:literal, $input:ident, $output:ident);)*) => {
        #[tonic::async_trait]
        impl v1::latch_agent_server::LatchAgent for GrpcService {
            $(async fn $method(&self, request: Request<v1::$input>) -> Result<Response<v1::$output>, Status> {
                self.execute(request, $alias, concat!("latch.v1.", stringify!($input)), concat!("latch.v1.", stringify!($output))).await
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
                                let value = serde_json::to_value(event).map_err(|_| Status::internal("event encoding failed"))?;
                                let message = codec::from_legacy(value, "latch.v1.AgentEvent").map_err(|e| codec::status(&e))?;
                                yield message;
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
