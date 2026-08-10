/// Why a repository operation failed, shared by every persistence port.
///
/// Variants say what happened to the operation, at the level a caller decides
/// on, in words that hold for any store.
///
/// Repositories never return domain errors: what a rollback means (rerun, or a
/// fact like "username taken") is decided by the application service, which has
/// the context.
///
/// No variant carries a source. The diagnosis is logged where the error is
/// born, and callers rely on the trace rather than re-logging.
///
/// Deliberately not `#[non_exhaustive]`: adding a variant should break every
/// `match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RepositoryError {
    /// Another transaction got in the way, and this unit of work rolls back.
    /// Rerunning it from fresh reads can succeed.
    #[error("rolled back")]
    RolledBack,

    /// The store did not answer: refused, timed out, or dropped mid-call. The
    /// outcome is unknown, including whether a commit landed, so nothing
    /// in-process reruns it. The client may try again later.
    #[error("persistence did not answer")]
    Unanswered,

    /// The store answered with a failure that will repeat: bad SQL, bad stored
    /// data, a bug. Not worth retrying by anyone.
    #[error("persistence failure")]
    Internal,
}
