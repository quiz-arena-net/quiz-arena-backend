use sea_orm::{
    DbErr, RuntimeErr, SqlErr,
    sqlx::{self, mysql::MySqlDatabaseError, sqlite::SqliteError},
};
use tracing::{debug, error, warn};

use crate::kernel::domain::RepositoryError;

/// Classifies a database error by what the store answered.
///
/// Pure. The diagnosis is logged by [`report`] where the error is born.
impl From<DbErr> for RepositoryError {
    fn from(source: DbErr) -> Self {
        classify(&source)
    }
}

/// Logs a database failure with the driver's detail, at a level matching its
/// classification. Call it where the error is born, before converting: the
/// decision crosses the boundary as the variant, and no caller re-logs.
pub fn report(source: &DbErr) {
    match classify(source) {
        RepositoryError::RolledBack => debug!(%source, "contended"),
        RepositoryError::Unanswered => warn!(%source, "persistence did not answer"),
        RepositoryError::Internal => error!(%source, "persistence failure"),
    }
}

fn classify(source: &DbErr) -> RepositoryError {
    if contended(source) {
        RepositoryError::RolledBack
    } else if unanswered(source) {
        RepositoryError::Unanswered
    } else {
        RepositoryError::Internal
    }
}

/// Contention the store reported. Each case also proves a failed `COMMIT` did
/// not land, which is what makes a rerun safe there.
fn contended(source: &DbErr) -> bool {
    if matches!(source.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        return true;
    }
    let Some(error) = database_error(source) else {
        return false;
    };
    // SQLSTATE 40001 is the standard serialization failure, which MySQL raises
    // for deadlocks.
    if error.code().is_some_and(|code| code == "40001") {
        return true;
    }
    if error.try_downcast_ref::<SqliteError>().is_some() {
        // SQLITE_BUSY (5) and SQLITE_LOCKED (6), raised at any statement
        // including COMMIT. The low byte also matches their extended codes.
        return error
            .code()
            .and_then(|code| code.parse::<u32>().ok())
            .is_some_and(|code| matches!(code & 0xFF, 5 | 6));
    }
    // ER_LOCK_WAIT_TIMEOUT. Its SQLSTATE is the generic HY000, so only the
    // MySQL error number identifies it.
    error
        .try_downcast_ref::<MySqlDatabaseError>()
        .is_some_and(|error| error.number() == 1205)
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

/// The driver-level error behind a SeaORM error, when there is one.
fn database_error(source: &DbErr) -> Option<&dyn sqlx::error::DatabaseError> {
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
