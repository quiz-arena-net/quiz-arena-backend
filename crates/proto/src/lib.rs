//! Generated protobuf messages and ConnectRPC service bindings for every
//! service in this workspace.
//!
//! Every service crate depends on this one, so a caller gets the client
//! bindings for the services it talks to alongside the server bindings for the
//! services it serves. Emitted into `$OUT_DIR` by `build.rs`. See `api/proto/`
//! for the sources.

::connectrpc::include_generated!();

// The generated tree is rooted at the `quiz_arena` proto package, which would
// otherwise repeat the crate name at every use site.
pub use quiz_arena::*;

/// Wire-format `FileDescriptorSet` for `api/proto/`, backing gRPC server
/// reflection. Emitted by `build.rs` via `emit_descriptor_set`.
///
/// It covers every service in the workspace, so a server reflects only the
/// subset it actually mounts. `quiz_arena_shared::server::serve` applies that
/// filter.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/quiz-arena.fds.bin"));
