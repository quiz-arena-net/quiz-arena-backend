use crate::modules::greet::domain::GreetingRepository;

/// The greet module's view of an open transaction: its repository ports, all
/// bound to one atomic unit of work.
pub(crate) trait GreetTransactionContext: Send + Sync {
    fn greetings(&self) -> &dyn GreetingRepository;
}
