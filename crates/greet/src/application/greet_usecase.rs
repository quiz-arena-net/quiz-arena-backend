use async_trait::async_trait;
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter},
};
use tracing::{Span, debug, info, warn};

use quiz_arena_shared::kernel::{application::UnitOfWork, domain::RepositoryError};

use crate::{
    application::transaction_context::GreetTransactionContext,
    domain::{AddGreetingRejection, Greeting, SenderName, SenderNameError},
};

/// Attempts per request before giving up on transient failures.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum GreetError {
    #[error("sender name must not be empty")]
    EmptySenderName,
    #[error("sender name is too long ({length} characters)")]
    SenderNameTooLong { length: usize },
    #[error("sender name must contain only letters and numbers")]
    SenderNameContainsInvalidCharacter,
    #[error("greeting encountered a concurrency conflict")]
    Conflict,
    #[error("failed to record greeting")]
    Internal,
}

impl From<SenderNameError> for GreetError {
    fn from(error: SenderNameError) -> Self {
        match error {
            SenderNameError::Empty => Self::EmptySenderName,
            SenderNameError::TooLong { length } => Self::SenderNameTooLong { length },
            SenderNameError::InvalidCharacter => Self::SenderNameContainsInvalidCharacter,
        }
    }
}

impl From<RepositoryError> for GreetError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::Conflict => Self::Conflict,
            RepositoryError::Internal => Self::Internal,
        }
    }
}

impl From<RepositoryError<AddGreetingRejection>> for GreetError {
    fn from(error: RepositoryError<AddGreetingRejection>) -> Self {
        match error {
            RepositoryError::Rejected(AddGreetingRejection::AlreadyExists) => {
                // Greet adds only after observing no history. An existing
                // history is a lost insert race, recoverable via fresh reads.
                Self::Conflict
            }
            RepositoryError::Conflict => Self::Conflict,
            RepositoryError::Internal => Self::Internal,
        }
    }
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
    pub(crate) fn new(unit_of_work: U, meter: Meter) -> Self {
        let greetings_recorded = meter
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

        // Retry only after the unit of work has abandoned the attempt, with
        // fresh reads and a fresh add-versus-update decision.
        let mut attempt = 1;
        let outcome = loop {
            let sender = sender.clone();
            let work = async move |context: &U::TransactionContext<'_>| {
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
            };

            let outcome = self
                .unit_of_work
                .run(move |context| Box::pin(work(context)))
                .await;

            match outcome {
                Err(error @ GreetError::Conflict) => {
                    if attempt >= MAX_ATTEMPTS {
                        warn!(%error, attempts = MAX_ATTEMPTS, "giving up on recording greeting");
                        break Err(error);
                    }
                    debug!(%error, attempt, "retrying greeting");
                    attempt += 1;
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

    use quiz_arena_shared::kernel::application::Work;

    use crate::domain::GreetingRepository;

    use super::*;

    /// Repository that makes the use case lose the insert race a set number of
    /// times: each losing attempt finds no row and conflicts on add, and once
    /// the losses are spent the winner's row is there to update.
    struct RacingRepository {
        conflicts: u32,
        add_error: RepositoryError<AddGreetingRejection>,
        attempts: Arc<AtomicU32>,
    }

    #[async_trait]
    impl GreetingRepository for RacingRepository {
        async fn find_by_sender(
            &self,
            sender: &SenderName,
        ) -> Result<Option<Greeting>, RepositoryError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt <= self.conflicts {
                Ok(None)
            } else {
                Ok(Some(Greeting::from_persistence(sender.clone(), 1)))
            }
        }

        async fn add(
            &self,
            _greeting: &Greeting,
        ) -> Result<(), RepositoryError<AddGreetingRejection>> {
            Err(self.add_error)
        }

        async fn update(&self, _greeting: &Greeting) -> Result<(), RepositoryError> {
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

        async fn run<T, E, F>(&self, work: F) -> Result<T, E>
        where
            T: Send,
            E: From<RepositoryError> + Send,
            F: for<'r, 'tx> FnOnce(&'r Self::TransactionContext<'tx>) -> Work<'r, T, E> + Send,
        {
            work(&self.context).await
        }
    }

    fn interactor(conflicts: u32) -> (GreetInteractor<RacingUnitOfWork>, Arc<AtomicU32>) {
        let attempts = Arc::new(AtomicU32::new(0));
        let unit_of_work = RacingUnitOfWork {
            context: RacingContext {
                greetings: RacingRepository {
                    conflicts,
                    add_error: RepositoryError::Rejected(AddGreetingRejection::AlreadyExists),
                    attempts: Arc::clone(&attempts),
                },
            },
        };
        // No provider is installed in tests, so this meter is the no-op one.
        let meter = opentelemetry::global::meter("test");
        (GreetInteractor::new(unit_of_work, meter), attempts)
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
    async fn reports_conflict_after_exhausting_insert_race_retries() {
        let (interactor, attempts) = interactor(u32::MAX);

        let outcome = interactor
            .execute(GreetInput {
                sender: "alice".to_owned(),
            })
            .await;

        assert!(matches!(outcome, Err(GreetError::Conflict)));
        assert_eq!(attempts.load(Ordering::SeqCst), MAX_ATTEMPTS);
    }

    #[tokio::test]
    async fn retries_concurrency_conflicts() {
        let (mut interactor, attempts) = interactor(1);
        interactor.unit_of_work.context.greetings.add_error = RepositoryError::Conflict;
        let output = interactor
            .execute(GreetInput {
                sender: "alice".to_owned(),
            })
            .await
            .unwrap();
        assert!(output.returning);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn preserves_conflict_after_exhausting_attempts() {
        let (mut interactor, attempts) = interactor(u32::MAX);
        interactor.unit_of_work.context.greetings.add_error = RepositoryError::Conflict;
        let outcome = interactor
            .execute(GreetInput {
                sender: "alice".to_owned(),
            })
            .await;
        assert!(matches!(outcome, Err(GreetError::Conflict)));
        assert_eq!(attempts.load(Ordering::SeqCst), MAX_ATTEMPTS);
    }

    #[tokio::test]
    async fn does_not_retry_internal_failures_with_potentially_unknown_outcomes() {
        let (mut interactor, attempts) = interactor(u32::MAX);
        interactor.unit_of_work.context.greetings.add_error = RepositoryError::Internal;
        let outcome = interactor
            .execute(GreetInput {
                sender: "alice".to_owned(),
            })
            .await;
        assert!(matches!(outcome, Err(GreetError::Internal)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_names_before_accessing_repositories() {
        for (sender, expected) in [
            (String::new(), GreetError::EmptySenderName),
            (
                "alice bob".to_owned(),
                GreetError::SenderNameContainsInvalidCharacter,
            ),
            (
                "あ".repeat(33),
                GreetError::SenderNameTooLong { length: 33 },
            ),
        ] {
            let (interactor, attempts) = interactor(0);
            let outcome = interactor.execute(GreetInput { sender }).await;
            assert_eq!(outcome.unwrap_err(), expected);
            assert_eq!(attempts.load(Ordering::SeqCst), 0);
        }
    }
}
