use async_trait::async_trait;
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    QuerySelect, RuntimeErr, Schema, SqlErr, sqlx,
};
use tracing::error;

use crate::modules::greet::domain::{
    Greeting, GreetingRepository, GreetingRepositoryError, SenderName,
};

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

/// Sorts a database error into the port's vocabulary: a conflict that justifies
/// rerunning the unit of work, or an opaque failure.
///
/// Only errors that prove the transaction did not commit count as conflicts: a
/// unique-key violation (two first greetings racing to insert) or SQLite's BUSY
/// and LOCKED write contention. Anything ambiguous stays `Other` so a retry can
/// never double-apply work.
impl From<DbErr> for GreetingRepositoryError {
    fn from(source: DbErr) -> Self {
        if matches!(source.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
            return Self::Conflict;
        }
        if let DbErr::Exec(RuntimeErr::SqlxError(error))
        | DbErr::Query(RuntimeErr::SqlxError(error)) = &source
            && let sqlx::Error::Database(error) = &**error
            && let Some(code) = error.code()
            && let Ok(code) = code.parse::<u32>()
            // SQLITE_BUSY (5) and SQLITE_LOCKED (6). The low byte also matches
            // their extended codes.
            && matches!(code & 0xFF, 5 | 6)
        {
            return Self::Conflict;
        }
        Self::Other(source.into())
    }
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
    ) -> Result<Option<Greeting>, GreetingRepositoryError> {
        let model = entity::Entity::find_by_id(sender.as_str())
            .lock_exclusive()
            .one(self.transaction)
            .await
            .map_err(|source| {
                let error = GreetingRepositoryError::from(source);
                if let GreetingRepositoryError::Other(source) = &error {
                    error!(%sender, %source, "failed to load greeting");
                }
                error
            })?;

        model
            .map(|model| {
                let sender = SenderName::new(model.sender).map_err(|source| {
                    error!(%source, "stored sender name violates domain rules");
                    GreetingRepositoryError::Other(source.into())
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
    async fn add(&self, greeting: &Greeting) -> Result<(), GreetingRepositoryError> {
        let model = entity::ActiveModel {
            sender: Set(greeting.sender().as_str().to_owned()),
            times_greeted: Set(greeting.times_greeted()),
        };

        entity::Entity::insert(model)
            .exec(self.transaction)
            .await
            .map_err(|source| {
                let error = GreetingRepositoryError::from(source);
                if let GreetingRepositoryError::Other(source) = &error {
                    error!(sender = %greeting.sender(), %source, "failed to add greeting");
                }
                error
            })?;
        Ok(())
    }

    #[tracing::instrument(
        name = "update_greeting",
        skip_all,
        fields(sender = %greeting.sender(), times_greeted = i64::from(greeting.times_greeted()))
    )]
    async fn update(&self, greeting: &Greeting) -> Result<(), GreetingRepositoryError> {
        let model = entity::ActiveModel {
            sender: Set(greeting.sender().as_str().to_owned()),
            times_greeted: Set(greeting.times_greeted()),
        };

        entity::Entity::update(model)
            .exec(self.transaction)
            .await
            .map_err(|source| {
                let error = GreetingRepositoryError::from(source);
                if let GreetingRepositoryError::Other(source) = &error {
                    error!(sender = %greeting.sender(), %source, "failed to update greeting");
                }
                error
            })?;
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
    use sea_orm::{ConnectOptions, Database, TransactionTrait};

    use super::*;

    async fn database() -> DatabaseConnection {
        // A single connection so every pooled checkout sees the same
        // in-memory database.
        let connect_options = ConnectOptions::new("sqlite::memory:")
            .max_connections(1)
            .to_owned();
        let database = Database::connect(connect_options).await.unwrap();
        create_schema(&database).await.unwrap();
        database
    }

    // Pins the insert-race behavior the port contract asks for: adding a sender
    // that already exists must surface as a retryable conflict, not overwrite
    // and not an opaque failure.
    #[tokio::test]
    async fn add_rejects_duplicate_sender_as_conflict() {
        let database = database().await;
        let transaction = database.begin().await.unwrap();
        let repository = SeaOrmGreetingRepository::new(&transaction);
        let greeting = Greeting::first(SenderName::new("alice").unwrap());

        repository.add(&greeting).await.unwrap();
        let outcome = repository.add(&greeting).await;

        assert!(matches!(outcome, Err(GreetingRepositoryError::Conflict)));
    }
}
