use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use tracing::error;

use crate::kernel::{
    application::{UnitOfWork, Work},
    domain::RepositoryError,
    infrastructure::sea_orm::report,
};

/// Builds a service's transaction context bound to an open transaction.
///
/// The one piece of a SeaORM unit of work that differs per service. Each service
/// implements this once for its context and plugs it into [`SeaOrmUnitOfWork`].
pub trait SeaOrmTransactionContextFactory: Send + Sync {
    type TransactionContext<'tx>: Send + Sync;

    fn bind<'tx>(&self, transaction: &'tx DatabaseTransaction) -> Self::TransactionContext<'tx>;
}

/// SeaORM-backed [`UnitOfWork`]: one database transaction per unit.
pub struct SeaOrmUnitOfWork<F> {
    database: DatabaseConnection,
    transaction_context_factory: F,
}

impl<F> SeaOrmUnitOfWork<F> {
    pub fn new(database: DatabaseConnection, transaction_context_factory: F) -> Self {
        Self {
            database,
            transaction_context_factory,
        }
    }
}

#[async_trait]
impl<F: SeaOrmTransactionContextFactory> UnitOfWork for SeaOrmUnitOfWork<F> {
    type TransactionContext<'tx> = F::TransactionContext<'tx>;

    #[tracing::instrument(name = "transaction", skip_all)]
    async fn run<T, E, W>(&self, work: W) -> Result<T, E>
    where
        T: Send,
        E: From<RepositoryError> + Send,
        W: for<'r, 'tx> FnOnce(&'r Self::TransactionContext<'tx>) -> Work<'r, T, E> + Send,
    {
        let transaction = self
            .database
            .begin()
            .await
            .inspect_err(report)
            .map_err(RepositoryError::from)?;

        let context = self.transaction_context_factory.bind(&transaction);
        let outcome = work(&context).await;
        drop(context);

        match outcome {
            Ok(value) => {
                transaction
                    .commit()
                    .await
                    .inspect_err(report)
                    .map_err(RepositoryError::from)?;
                Ok(value)
            }
            Err(error) => {
                // Dropping the transaction would also roll back. Explicit so
                // a rollback failure surfaces in the logs.
                if let Err(source) = transaction.rollback().await {
                    error!(%source, "failed to roll back transaction");
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveValue::Set, ConnectionTrait, Database, EntityTrait, Schema};

    use super::*;

    mod entity {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
        #[sea_orm(table_name = "notes")]
        pub(super) struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub id: i32,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub(super) enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    struct Notes<'tx> {
        transaction: &'tx DatabaseTransaction,
    }

    impl Notes<'_> {
        async fn add(&self, id: i32) -> Result<(), RepositoryError> {
            entity::Entity::insert(entity::ActiveModel { id: Set(id) })
                .exec(self.transaction)
                .await
                .inspect_err(report)?;
            Ok(())
        }

        async fn ids(&self) -> Result<Vec<i32>, RepositoryError> {
            let models = entity::Entity::find()
                .all(self.transaction)
                .await
                .inspect_err(report)?;
            Ok(models.into_iter().map(|model| model.id).collect())
        }
    }

    struct NotesFactory;

    impl SeaOrmTransactionContextFactory for NotesFactory {
        type TransactionContext<'tx> = Notes<'tx>;

        fn bind<'tx>(&self, transaction: &'tx DatabaseTransaction) -> Notes<'tx> {
            Notes { transaction }
        }
    }

    /// Work error with a domain variant of its own, the shape a use case
    /// takes when its work can fail beyond its repositories.
    #[derive(Debug, PartialEq, Eq, thiserror::Error)]
    enum NoteError {
        #[error("work failed")]
        WorkFailed,
        #[error(transparent)]
        Repository(#[from] RepositoryError),
    }

    async fn unit_of_work() -> SeaOrmUnitOfWork<NotesFactory> {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        let statement =
            Schema::new(database.get_database_backend()).create_table_from_entity(entity::Entity);
        database.execute(&statement).await.unwrap();
        SeaOrmUnitOfWork::new(database, NotesFactory)
    }

    async fn stored_ids(unit_of_work: &SeaOrmUnitOfWork<NotesFactory>) -> Vec<i32> {
        unit_of_work
            .run(|notes| Box::pin(async move { notes.ids().await }))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn commits_when_work_succeeds() {
        let unit_of_work = unit_of_work().await;

        unit_of_work
            .run(|notes| {
                Box::pin(async move {
                    notes.add(1).await?;
                    notes.add(2).await
                })
            })
            .await
            .unwrap();

        assert_eq!(stored_ids(&unit_of_work).await, vec![1, 2]);
    }

    #[tokio::test]
    async fn rolls_back_when_work_fails() {
        let unit_of_work = unit_of_work().await;

        let outcome = unit_of_work
            .run(|notes| {
                Box::pin(async move {
                    notes.add(1).await?;
                    Err::<(), _>(NoteError::WorkFailed)
                })
            })
            .await;

        assert_eq!(outcome, Err(NoteError::WorkFailed));
        assert_eq!(stored_ids(&unit_of_work).await, Vec::<i32>::new());
    }

    /// A unique violation inside the work must reach the caller as a rollback
    /// rather than an opaque failure.
    #[tokio::test]
    async fn reports_a_duplicate_insert_as_rolled_back() {
        let unit_of_work = unit_of_work().await;

        let outcome = unit_of_work
            .run(|notes| {
                Box::pin(async move {
                    notes.add(1).await?;
                    notes.add(1).await
                })
            })
            .await;

        assert_eq!(outcome, Err(RepositoryError::RolledBack));
    }
}
