use std::sync::Arc;

use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};

use quiz_arena_proto::greet::v1::{GreetRequest, GreetResponse, GreetService};

use crate::application::{GreetError, GreetInput, GreetUsecase};

impl From<GreetError> for ConnectError {
    fn from(error: GreetError) -> Self {
        match error {
            GreetError::EmptySenderName
            | GreetError::SenderNameTooLong { .. }
            | GreetError::SenderNameContainsInvalidCharacter => {
                Self::invalid_argument(error.to_string())
            }
            GreetError::Conflict => Self::aborted("contended, retry"),
            GreetError::Internal => Self::internal("internal error"),
        }
    }
}

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
        let output = self.greet_usecase.execute(input).await?;

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
