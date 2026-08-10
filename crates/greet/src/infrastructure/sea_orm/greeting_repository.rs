use async_trait::async_trait;
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    QuerySelect, Schema, SqlErr,
};
use tracing::{debug, error};

use quiz_arena_shared::kernel::{domain::RepositoryError, infrastructure::log_database_error};

use crate::domain::{AddGreetingRejection, Greeting, GreetingRepository, SenderName};

mod entity {
    use sea_orm::entity::prelude::*;

    /// Database row for a sender's greeting history.
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "greetings")]
    pub(super) struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[sea_orm(column_type = "String(StringLen::N(32))")]
        pub sender: String,
        /// Stored signed because Postgres has no unsigned integers.
        pub times_greeted: i64,
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
            .inspect_err(log_database_error)?;

        model
            .map(|model| {
                let sender = SenderName::new(model.sender).map_err(|source| {
                    error!(%source, "stored sender name violates domain rules");
                    RepositoryError::Internal
                })?;
                let times_greeted = u32::try_from(model.times_greeted).map_err(|source| {
                    error!(%source, "stored greeting count violates domain rules");
                    RepositoryError::Internal
                })?;
                Ok(Greeting::from_persistence(sender, times_greeted))
            })
            .transpose()
    }

    #[tracing::instrument(
        name = "add_greeting",
        skip_all,
        fields(sender = %greeting.sender(), times_greeted = i64::from(greeting.times_greeted()))
    )]
    async fn add(&self, greeting: &Greeting) -> Result<(), RepositoryError<AddGreetingRejection>> {
        let model = entity::ActiveModel {
            sender: Set(greeting.sender().as_str().to_owned()),
            times_greeted: Set(i64::from(greeting.times_greeted())),
        };

        entity::Entity::insert(model)
            .exec(self.transaction)
            .await
            .map_err(|source| {
                // Sender identity is this table's only unique key.
                if matches!(source.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                    debug!(%source, "greeting history already exists");
                    RepositoryError::Rejected(AddGreetingRejection::AlreadyExists)
                } else {
                    log_database_error(&source);
                    source.into()
                }
            })?;
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
            times_greeted: Set(i64::from(greeting.times_greeted())),
        };

        entity::Entity::update(model)
            .exec(self.transaction)
            .await
            .inspect_err(log_database_error)?;
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
