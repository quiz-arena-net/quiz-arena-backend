use quiz_arena_proto::registry::v1::QuizAuthoringService;

/// ConnectRPC handler for `quiz_arena.registry.v1.QuizAuthoringService`.
pub(crate) struct QuizAuthoringServiceHandler {}

impl QuizAuthoringServiceHandler {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl QuizAuthoringService for QuizAuthoringServiceHandler {}
