use async_trait::async_trait;
use opentelemetry::{KeyValue, global, metrics::Counter};
use tracing::{Span, debug, info, warn};

use crate::{
    modules::greet::{
        application::transaction_context::GreetTransactionContext,
        domain::{Greeting, GreetingRepositoryError, SenderName, SenderNameError},
    },
    shared::application::{UnitOfWork, UnitOfWorkError, boxed_work},
};

/// Attempts per request before giving up on conflicts.
///
/// One retry already resolves the deterministic case (the loser of an insert
/// race flips to the update path), the rest absorb fresh contention.
const MAX_ATTEMPTS: u32 = 3;

#[derive(Debug)]
pub(crate) struct GreetInput {
    pub sender: String,
}

#[derive(Debug)]
pub(crate) struct GreetOutput {
    pub sender: String,
    pub returning: bool,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum GreetError {
    /// The sender name violates domain rules, so the request is invalid.
    #[error("invalid sender name: {0}")]
    InvalidSenderName(#[from] SenderNameError),
    /// The greeting could not be recorded.
    #[error("failed to record greeting")]
    Persistence(#[from] UnitOfWorkError<GreetingRepositoryError>),
}

/// Records a greeting from a sender and reports their greeting history.
#[async_trait]
pub(crate) trait GreetUsecase: Send + Sync {
    async fn execute(&self, input: GreetInput) -> Result<GreetOutput, GreetError>;
}

pub(crate) struct GreetInteractor<U> {
    unit_of_work: U,
    greetings_recorded: Counter<u64>,
}

impl<U> GreetInteractor<U> {
    pub(crate) fn new(unit_of_work: U) -> Self {
        let greetings_recorded = global::meter("quiz_arena.greet")
            .u64_counter("greet.greetings_recorded")
            .with_description("Greetings recorded, split by first-time and returning senders.")
            .build();
        Self {
            unit_of_work,
            greetings_recorded,
        }
    }
}

#[async_trait]
impl<U> GreetUsecase for GreetInteractor<U>
where
    U: UnitOfWork,
    for<'tx> U::TransactionContext<'tx>: GreetTransactionContext,
{
    #[tracing::instrument(
        name = "greet_usecase",
        skip_all,
        fields(returning, times_greeted, attempts)
    )]
    async fn execute(&self, input: GreetInput) -> Result<GreetOutput, GreetError> {
        let sender = SenderName::new(input.sender)?;

        // A conflict means the transaction rolled back, so each new attempt
        // starts from scratch: fresh transaction, fresh reads, and the
        // add-versus-update decision made again from what it read.
        let mut attempt = 1;
        let outcome = loop {
            let sender = sender.clone();
            let outcome = self
                .unit_of_work
                .run(move |context| {
                    boxed_work(async move {
                        let greetings = context.greetings();
                        let greeting = match greetings.find_by_sender(&sender).await? {
                            Some(mut greeting) => {
                                greeting.record_another();
                                greetings.update(&greeting).await?;
                                greeting
                            }
                            None => {
                                let greeting = Greeting::first(sender);
                                greetings.add(&greeting).await?;
                                greeting
                            }
                        };
                        Ok(greeting)
                    })
                })
                .await;

            match outcome {
                Err(UnitOfWorkError::Work(GreetingRepositoryError::Conflict))
                    if attempt < MAX_ATTEMPTS =>
                {
                    debug!(
                        attempt,
                        "greeting conflicted with a concurrent writer, retrying"
                    );
                    attempt += 1;
                }
                Err(error @ UnitOfWorkError::Work(GreetingRepositoryError::Conflict)) => {
                    warn!(attempts = MAX_ATTEMPTS, "giving up on conflicted greeting");
                    break Err(error);
                }
                outcome => break outcome,
            }
        };
        Span::current().record("attempts", i64::from(attempt));
        let greeting = outcome?;

        Span::current().record("returning", greeting.is_returning());
        Span::current().record("times_greeted", i64::from(greeting.times_greeted()));
        self.greetings_recorded
            .add(1, &[KeyValue::new("returning", greeting.is_returning())]);
        info!(
            sender = %greeting.sender(),
            returning = greeting.is_returning(),
            "recorded greeting"
        );

        Ok(GreetOutput {
            sender: greeting.sender().to_string(),
            returning: greeting.is_returning(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    use crate::{modules::greet::domain::GreetingRepository, shared::application::Work};

    use super::*;

    /// Repository that makes the use case lose the insert race a set number of
    /// times: each losing attempt finds no row and conflicts on add, and once
    /// the losses are spent the winner's row is there to update.
    struct RacingRepository {
        conflicts: u32,
        attempts: Arc<AtomicU32>,
    }

    #[async_trait]
    impl GreetingRepository for RacingRepository {
        async fn find_by_sender(
            &self,
            sender: &SenderName,
        ) -> Result<Option<Greeting>, GreetingRepositoryError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt <= self.conflicts {
                Ok(None)
            } else {
                Ok(Some(Greeting::from_persistence(sender.clone(), 1)))
            }
        }

        async fn add(&self, _greeting: &Greeting) -> Result<(), GreetingRepositoryError> {
            Err(GreetingRepositoryError::Conflict)
        }

        async fn update(&self, _greeting: &Greeting) -> Result<(), GreetingRepositoryError> {
            Ok(())
        }
    }

    struct RacingContext {
        greetings: RacingRepository,
    }

    impl GreetTransactionContext for RacingContext {
        fn greetings(&self) -> &dyn GreetingRepository {
            &self.greetings
        }
    }

    /// Unit of work without a database: hands the work its context and reports
    /// the work's error, which is all the retry loop observes.
    struct RacingUnitOfWork {
        context: RacingContext,
    }

    #[async_trait]
    impl UnitOfWork for RacingUnitOfWork {
        type TransactionContext<'tx> = RacingContext;

        async fn run<T, E, F>(&self, work: F) -> Result<T, UnitOfWorkError<E>>
        where
            T: Send,
            E: Send,
            F: for<'r, 'tx> FnOnce(&'r Self::TransactionContext<'tx>) -> Work<'r, T, E> + Send,
        {
            work(&self.context).await.map_err(UnitOfWorkError::Work)
        }
    }

    fn interactor(conflicts: u32) -> (GreetInteractor<RacingUnitOfWork>, Arc<AtomicU32>) {
        let attempts = Arc::new(AtomicU32::new(0));
        let unit_of_work = RacingUnitOfWork {
            context: RacingContext {
                greetings: RacingRepository {
                    conflicts,
                    attempts: Arc::clone(&attempts),
                },
            },
        };
        (GreetInteractor::new(unit_of_work), attempts)
    }

    #[tokio::test]
    async fn retries_and_takes_the_update_path_after_losing_the_insert_race() {
        let (interactor, attempts) = interactor(1);

        let output = interactor
            .execute(GreetInput {
                sender: "alice".to_owned(),
            })
            .await
            .unwrap();

        // The second attempt re-read, saw the winner's row, and flipped from
        // the add branch to the update branch.
        assert!(output.returning);
        assert_eq!(output.sender, "alice");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn gives_up_after_exhausting_attempts() {
        let (interactor, attempts) = interactor(u32::MAX);

        let outcome = interactor
            .execute(GreetInput {
                sender: "alice".to_owned(),
            })
            .await;

        assert!(matches!(
            outcome,
            Err(GreetError::Persistence(UnitOfWorkError::Work(
                GreetingRepositoryError::Conflict
            )))
        ));
        assert_eq!(attempts.load(Ordering::SeqCst), MAX_ATTEMPTS);
    }
}
