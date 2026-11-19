use quiz_arena_proto::gameplay::v1::GameplayService;

/// ConnectRPC handler for `quiz_arena.gameplay.v1.GameplayService`.
pub(crate) struct GameplayServiceHandler {}

impl GameplayServiceHandler {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl GameplayService for GameplayServiceHandler {}
