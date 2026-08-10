//! Generated protobuf messages and ConnectRPC service bindings.
//!
//! Emitted into `$OUT_DIR` by `build.rs`. See `api/proto/` for the sources.

::connectrpc::include_generated!();

/// Wire-format `FileDescriptorSet` for `api/proto/`, backing gRPC server
/// reflection. Emitted by `build.rs` via `emit_descriptor_set`.
pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/quiz-arena-backend.fds.bin"));
