use std::sync::Arc;

use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};

use quiz_arena_proto::greet::v1::{GreetRequest, GreetResponse, GreetService};

use crate::application::{GreetError, GreetInput, GreetUsecase};

/// ConnectRPC handler for `quiz_arena.greet.v1.GreetService`.
pub(crate) struct GreetServiceHandler {
    greet_usecase: Arc<dyn GreetUsecase>,
}

impl GreetServiceHandler {
    pub(crate) fn new(greet_usecase: Arc<dyn GreetUsecase>) -> Self {
        Self { greet_usecase }
    }
}

impl GreetService for GreetServiceHandler {
    #[tracing::instrument(name = "greet_handler", skip_all, fields(sender = %request.sender))]
    async fn greet<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GreetRequest>,
    ) -> ServiceResult<impl Encodable<GreetResponse> + Send + use<'a>> {
        let input = GreetInput {
            sender: request.sender.to_owned(),
        };
        let output = match self.greet_usecase.execute(input).await {
            Ok(output) => output,
            Err(error @ GreetError::InvalidSenderName(_)) => {
                return Err(ConnectError::invalid_argument(error.to_string()));
            }
            Err(GreetError::Persistence(error)) => return Err(error.into()),
        };

        let message = if output.returning {
            format!("Welcome back, {}!", output.sender)
        } else {
            format!("Hello, {}! Nice to meet you.", output.sender)
        };

        Response::ok(GreetResponse {
            message,
            ..Default::default()
        })
    }
}
