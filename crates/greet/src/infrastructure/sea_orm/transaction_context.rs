use sea_orm::DatabaseTransaction;

use quiz_arena_shared::kernel::infrastructure::SeaOrmTransactionContextFactory;

use crate::{application::GreetTransactionContext, domain::GreetingRepository};

use super::greeting_repository::SeaOrmGreetingRepository;

/// The greet service's repositories, all borrowing the same open transaction.
pub(crate) struct SeaOrmGreetTransactionContext<'tx> {
    greetings: SeaOrmGreetingRepository<'tx>,
}

impl GreetTransactionContext for SeaOrmGreetTransactionContext<'_> {
    fn greetings(&self) -> &dyn GreetingRepository {
        &self.greetings
    }
}

/// Binds the greet service's transaction context to each unit of work's
/// transaction.
pub(crate) struct SeaOrmGreetTransactionContextFactory;

impl SeaOrmTransactionContextFactory for SeaOrmGreetTransactionContextFactory {
    type TransactionContext<'tx> = SeaOrmGreetTransactionContext<'tx>;

    fn bind<'tx>(
        &self,
        transaction: &'tx DatabaseTransaction,
    ) -> SeaOrmGreetTransactionContext<'tx> {
        SeaOrmGreetTransactionContext {
            greetings: SeaOrmGreetingRepository::new(transaction),
        }
    }
}
