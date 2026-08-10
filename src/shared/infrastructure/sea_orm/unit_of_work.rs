use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use tracing::error;

use crate::shared::application::{UnitOfWork, UnitOfWorkError, Work};

/// Builds a module's transaction context bound to an open transaction.
///
/// The one piece of a SeaORM unit of work that differs per module. Each module
/// implements this once for its context and plugs it into [`SeaOrmUnitOfWork`].
pub(crate) trait SeaOrmTransactionContextFactory: Send + Sync {
    type TransactionContext<'tx>: Send + Sync;

    fn bind<'tx>(&self, transaction: &'tx DatabaseTransaction) -> Self::TransactionContext<'tx>;
}

/// SeaORM-backed [`UnitOfWork`]: one database transaction per unit.
pub(crate) struct SeaOrmUnitOfWork<F> {
    database: DatabaseConnection,
    transaction_context_factory: F,
}

impl<F> SeaOrmUnitOfWork<F> {
    pub(crate) fn new(database: DatabaseConnection, transaction_context_factory: F) -> Self {
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
    async fn run<T, E, W>(&self, work: W) -> Result<T, UnitOfWorkError<E>>
    where
        T: Send,
        E: Send,
        W: for<'r, 'tx> FnOnce(&'r Self::TransactionContext<'tx>) -> Work<'r, T, E> + Send,
    {
        let transaction = self.database.begin().await.map_err(|source| {
            error!(%source, "failed to begin transaction");
            UnitOfWorkError::Transaction(source.into())
        })?;

        let context = self.transaction_context_factory.bind(&transaction);
        let outcome = work(&context).await;
        drop(context);

        match outcome {
            Ok(value) => {
                transaction.commit().await.map_err(|source| {
                    error!(%source, "failed to commit transaction");
                    UnitOfWorkError::Transaction(source.into())
                })?;
                Ok(value)
            }
            Err(error) => {
                // Dropping the transaction would also roll back. Explicit so
                // a rollback failure surfaces in the logs.
                if let Err(source) = transaction.rollback().await {
                    error!(%source, "failed to roll back transaction");
                }
                Err(UnitOfWorkError::Work(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{
        ActiveValue::Set, ConnectOptions, ConnectionTrait, Database, DbErr, EntityTrait, Schema,
    };

    use crate::shared::application::boxed_work;

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
        async fn add(&self, id: i32) -> Result<(), DbErr> {
            entity::Entity::insert(entity::ActiveModel { id: Set(id) })
                .exec(self.transaction)
                .await?;
            Ok(())
        }

        async fn ids(&self) -> Result<Vec<i32>, DbErr> {
            let models = entity::Entity::find().all(self.transaction).await?;
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

    #[derive(Debug, PartialEq, Eq, thiserror::Error)]
    #[error("work failed")]
    struct WorkFailed;

    async fn unit_of_work() -> SeaOrmUnitOfWork<NotesFactory> {
        // A single connection so every pooled checkout sees the same
        // in-memory database.
        let connect_options = ConnectOptions::new("sqlite::memory:")
            .max_connections(1)
            .to_owned();
        let database = Database::connect(connect_options).await.unwrap();
        let statement =
            Schema::new(database.get_database_backend()).create_table_from_entity(entity::Entity);
        database.execute(&statement).await.unwrap();
        SeaOrmUnitOfWork::new(database, NotesFactory)
    }

    async fn stored_ids(unit_of_work: &SeaOrmUnitOfWork<NotesFactory>) -> Vec<i32> {
        unit_of_work
            .run(|notes| boxed_work(async move { notes.ids().await }))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn commits_when_work_succeeds() {
        let unit_of_work = unit_of_work().await;

        unit_of_work
            .run(|notes| {
                boxed_work(async move {
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
                boxed_work(async move {
                    notes
                        .add(1)
                        .await
                        .expect("insert inside the transaction should succeed");
                    Err::<(), _>(WorkFailed)
                })
            })
            .await;

        assert!(matches!(outcome, Err(UnitOfWorkError::Work(WorkFailed))));
        assert_eq!(stored_ids(&unit_of_work).await, Vec::<i32>::new());
    }
}
