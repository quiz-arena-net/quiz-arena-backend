use std::{future::Future, pin::Pin};

use async_trait::async_trait;

use crate::kernel::domain::RepositoryError;

/// Future produced by transactional work, borrowing the transaction context it
/// was handed.
///
/// Boxed because Rust cannot yet name "an async closure whose future borrows
/// its argument and is `Send`".
///
/// [`boxed_work`] keeps call sites tidy.
pub type Work<'r, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'r>>;

/// Port for running work as one atomic unit.
///
/// The unit commits when the work returns `Ok` and rolls back when it returns
/// `Err`, so a partial write can never escape a failed use case. Work receives
/// its repositories through the transaction context the unit hands it instead
/// of holding its own, which is what guarantees every operation joins the same
/// transaction. The repository ports themselves stay transaction-unaware.
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// The service's view of the open transaction: its repositories, all
    /// borrowing that transaction. Each service constrains this to its own
    /// context trait.
    type TransactionContext<'tx>: Send + Sync;

    /// Runs `work` in one transaction, committing it if the work succeeds.
    ///
    /// `E` is the work's own error type. It must absorb [`RepositoryError`]
    /// because beginning and committing the transaction can fail like any other
    /// persistence operation. Work that fails only through its repositories
    /// uses `RepositoryError` itself as `E`.
    //
    // `'r` (the work's borrow of the context) is distinct from `'tx` (the
    // context's borrow of the transaction): unifying them would borrow the
    // context for its own lifetime, leaving the implementation unable to
    // drop it and commit.
    async fn run<T, E, F>(&self, work: F) -> Result<T, E>
    where
        T: Send,
        E: From<RepositoryError> + Send,
        F: for<'r, 'tx> FnOnce(&'r Self::TransactionContext<'tx>) -> Work<'r, T, E> + Send;
}
