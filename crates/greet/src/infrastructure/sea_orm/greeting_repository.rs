use async_trait::async_trait;
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    QuerySelect, Schema,
};
use tracing::error;

use quiz_arena_shared::kernel::{domain::RepositoryError, infrastructure::report};

use crate::domain::{Greeting, GreetingRepository, SenderName};

mod entity {
    use sea_orm::entity::prelude::*;

    /// Database row for a sender's greeting history.
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "greetings")]
    pub(super) struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub sender: String,
        pub times_greeted: u32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub(super) enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// SeaORM-backed [`GreetingRepository`], bound to an open transaction.
pub(super) struct SeaOrmGreetingRepository<'tx> {
    transaction: &'tx DatabaseTransaction,
}

impl<'tx> SeaOrmGreetingRepository<'tx> {
    pub(super) fn new(transaction: &'tx DatabaseTransaction) -> Self {
        Self { transaction }
    }
}

#[async_trait]
impl GreetingRepository for SeaOrmGreetingRepository<'_> {
    #[tracing::instrument(name = "find_greeting", skip_all, fields(sender = %sender))]
    async fn find_by_sender(
        &self,
        sender: &SenderName,
    ) -> Result<Option<Greeting>, RepositoryError> {
        let model = entity::Entity::find_by_id(sender.as_str())
            .lock_exclusive()
            .one(self.transaction)
            .await
            .inspect_err(report)?;

        model
            .map(|model| {
                let sender = SenderName::new(model.sender).map_err(|source| {
                    error!(%source, "stored sender name violates domain rules");
                    RepositoryError::Internal
                })?;
                Ok(Greeting::from_persistence(sender, model.times_greeted))
            })
            .transpose()
    }

    #[tracing::instrument(
        name = "add_greeting",
        skip_all,
        fields(sender = %greeting.sender(), times_greeted = i64::from(greeting.times_greeted()))
    )]
    async fn add(&self, greeting: &Greeting) -> Result<(), RepositoryError> {
        let model = entity::ActiveModel {
            sender: Set(greeting.sender().as_str().to_owned()),
            times_greeted: Set(greeting.times_greeted()),
        };

        entity::Entity::insert(model)
            .exec(self.transaction)
            .await
            .inspect_err(report)?;
        Ok(())
    }

    #[tracing::instrument(
        name = "update_greeting",
        skip_all,
        fields(sender = %greeting.sender(), times_greeted = i64::from(greeting.times_greeted()))
    )]
    async fn update(&self, greeting: &Greeting) -> Result<(), RepositoryError> {
        let model = entity::ActiveModel {
            sender: Set(greeting.sender().as_str().to_owned()),
            times_greeted: Set(greeting.times_greeted()),
        };

        entity::Entity::update(model)
            .exec(self.transaction)
            .await
            .inspect_err(report)?;
        Ok(())
    }
}

/// Creates the table [`SeaOrmGreetingRepository`] reads and writes.
///
/// For databases that start empty. Databases with managed schemas migrate
/// outside the application instead.
pub(crate) async fn create_schema(database: &DatabaseConnection) -> Result<(), DbErr> {
    let statement = Schema::new(database.get_database_backend())
        .create_table_from_entity(entity::Entity)
        .if_not_exists()
        .to_owned();
    database.execute(&statement).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, TransactionTrait};

    use super::*;

    async fn database() -> DatabaseConnection {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        create_schema(&database).await.unwrap();
        database
    }

    // Pins the insert-race behavior the port contract asks for: adding a sender
    // that already exists must surface as a rollback, not overwrite and not an
    // opaque failure.
    #[tokio::test]
    async fn add_rejects_duplicate_sender_as_rolled_back() {
        let database = database().await;
        let transaction = database.begin().await.unwrap();
        let repository = SeaOrmGreetingRepository::new(&transaction);
        let greeting = Greeting::first(SenderName::new("alice").unwrap());

        repository.add(&greeting).await.unwrap();
        let outcome = repository.add(&greeting).await;

        assert_eq!(outcome, Err(RepositoryError::RolledBack));
    }
}
