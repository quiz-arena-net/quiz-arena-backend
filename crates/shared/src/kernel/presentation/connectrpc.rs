use connectrpc::ConnectError;

use crate::kernel::domain::RepositoryError;

/// The one place persistence outcomes become status codes. Handlers match on
/// their use case's own error and hand this variant through as is.
///
/// Messages are deliberately generic: the diagnosis is in the trace, not in the
/// response.
impl From<RepositoryError> for ConnectError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::RolledBack => Self::aborted("contended, retry"),
            RepositoryError::Unanswered => Self::unavailable("persistence unavailable"),
            RepositoryError::Internal => Self::internal("internal error"),
        }
    }
}
