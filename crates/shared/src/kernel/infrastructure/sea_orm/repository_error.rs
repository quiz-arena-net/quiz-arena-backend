use sea_orm::{
    DbErr, RuntimeErr,
    sqlx::{self, error::DatabaseError, postgres::PgDatabaseError, sqlite::SqliteError},
};
use tracing::{debug, error, warn};

use crate::kernel::domain::RepositoryError;

/// Converts unexpected failures and concurrency conflicts. Adapters translate
/// recognized operation rejections before using this fallback.
///
/// Pure. The diagnosis is logged by [`log_database_error`] where the error is
/// born.
impl<R> From<DbErr> for RepositoryError<R> {
    fn from(source: DbErr) -> Self {
        if contended(&source) {
            Self::Conflict
        } else {
            Self::Internal
        }
    }
}

/// Logs a database failure with the driver's detail, at a level matching its
/// classification. Call it where the error is born, before converting: the
/// decision crosses the boundary as the variant, and no caller re-logs.
pub fn log_database_error(source: &DbErr) {
    if contended(source) {
        debug!(%source, "contended");
    } else if unanswered(source) {
        warn!(%source, "persistence did not answer");
    } else {
        error!(%source, "persistence failure");
    }
}

/// Contention the store reported. Each case also proves a failed `COMMIT` did
/// not land, which is what makes a rerun safe there.
fn contended(source: &DbErr) -> bool {
    database_error(source).is_some_and(|error| sqlite_contended(error) || postgres_contended(error))
}

/// The driver-level error behind a SeaORM error, when there is one.
fn database_error(source: &DbErr) -> Option<&dyn DatabaseError> {
    if let DbErr::Exec(RuntimeErr::SqlxError(error))
    | DbErr::Query(RuntimeErr::SqlxError(error))
    | DbErr::Conn(RuntimeErr::SqlxError(error)) = source
        && let sqlx::Error::Database(error) = &**error
    {
        Some(&**error)
    } else {
        None
    }
}

/// SQLITE_BUSY (5) and SQLITE_LOCKED (6), raised at any statement including
/// COMMIT. The low byte also matches their extended codes.
fn sqlite_contended(error: &dyn DatabaseError) -> bool {
    error
        .try_downcast_ref::<SqliteError>()
        .and_then(|error| error.code()?.parse::<u32>().ok())
        .is_some_and(|code| matches!(code & 0xFF, 5 | 6))
}

/// SQLSTATE 40001 serialization failure, 40P01 deadlock detected, and 55P03
/// lock not available (`lock_timeout` or `NOWAIT`).
fn postgres_contended(error: &dyn DatabaseError) -> bool {
    error
        .try_downcast_ref::<PgDatabaseError>()
        .is_some_and(|error| matches!(error.code(), "40001" | "40P01" | "55P03"))
}

/// Whether the store could not be reached or did not answer in time.
fn unanswered(source: &DbErr) -> bool {
    match source {
        DbErr::Conn(_) | DbErr::ConnectionAcquire(_) => true,
        DbErr::Exec(RuntimeErr::SqlxError(error)) | DbErr::Query(RuntimeErr::SqlxError(error)) => {
            matches!(&**error, sqlx::Error::Io(_) | sqlx::Error::WorkerCrashed)
        }
        _ => false,
    }
}
