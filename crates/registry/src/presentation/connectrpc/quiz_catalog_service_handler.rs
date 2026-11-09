use quiz_arena_proto::registry::v1::QuizCatalogService;

/// ConnectRPC handler for `quiz_arena.registry.v1.QuizCatalogService`.
pub(crate) struct QuizCatalogServiceHandler {}

impl QuizCatalogServiceHandler {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl QuizCatalogService for QuizCatalogServiceHandler {}
