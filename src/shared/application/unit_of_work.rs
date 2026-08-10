use std::{error::Error, future::Future, pin::Pin};

use async_trait::async_trait;

/// Future produced by transactional work, borrowing the transaction context it
/// was handed.
///
/// Boxed because Rust cannot yet name "an async closure whose future borrows
/// its argument and is `Send`".
///
/// [`boxed_work`] keeps call sites tidy.
pub(crate) type Work<'r, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'r>>;

/// Port for running work as one atomic unit.
///
/// The unit commits when the work returns `Ok` and rolls back when it returns
/// `Err`, so a partial write can never escape a failed use case. Work receives
/// its repositories through the transaction context the unit hands it instead
/// of holding its own, which is what guarantees every operation joins the same
/// transaction. The repository ports themselves stay transaction-unaware.
#[async_trait]
pub(crate) trait UnitOfWork: Send + Sync {
    /// The module's view of the open transaction: its repositories, all
    /// borrowing that transaction. Each module constrains this to its own
    /// context trait.
    type TransactionContext<'tx>: Send + Sync;

    // `'r` (the work's borrow of the context) is distinct from `'tx` (the
    // context's borrow of the transaction): unifying them would borrow the
    // context for its own lifetime, leaving the implementation unable to
    // drop it and commit.
    async fn run<T, E, F>(&self, work: F) -> Result<T, UnitOfWorkError<E>>
    where
        T: Send,
        E: Send,
        F: for<'r, 'tx> FnOnce(&'r Self::TransactionContext<'tx>) -> Work<'r, T, E> + Send;
}

/// Why a unit of work did not commit.
#[derive(Debug, thiserror::Error)]
pub(crate) enum UnitOfWorkError<E> {
    /// The work itself failed, and its writes were rolled back.
    #[error(transparent)]
    Work(E),
    /// The transaction could not begin or commit. Opaque because callers cannot
    /// act on the detail. Implementations own it and log the failure before
    /// returning.
    #[error("transaction failed")]
    Transaction(#[source] Box<dyn Error + Send + Sync>),
}

/// Boxes the future produced by transactional work. See [`Work`].
pub(crate) fn boxed_work<'r, T, E>(
    future: impl Future<Output = Result<T, E>> + Send + 'r,
) -> Work<'r, T, E> {
    Box::pin(future)
}
