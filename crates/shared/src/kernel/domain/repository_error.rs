/// Why a repository operation failed, shared by every persistence port.
///
/// Ports define expected operation rejections through `R`. The default `!`
/// means the operation has no expected rejections. Application services
/// translate these outcomes into their own errors and decide how to recover.
///
/// Driver diagnostics stay in infrastructure logs. Rollback is the unit of
/// work's response to failure, not a failure reason.
///
/// Deliberately exhaustive: adding a variant should break every `match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RepositoryError<R = !> {
    /// An expected rejection defined by the repository port.
    #[error("{0}")]
    Rejected(R),

    /// Concurrent activity prevented completion. A fresh unit of work may
    /// succeed. Retry only after the previous attempt cannot commit.
    #[error("concurrency conflict")]
    Conflict,

    /// An unexpected failure. The outcome may be unknown, including whether a
    /// commit landed. Callers must not assume rollback or blindly retry.
    #[error("persistence failure")]
    Internal,
}
