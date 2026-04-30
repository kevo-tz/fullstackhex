use sqlx::PgPool;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database not configured")]
    NotConfigured,
    #[error("pool acquire timeout after {0:?}")]
    PoolTimeout(Duration),
    #[error("query failed: {0}")]
    QueryFailed(#[from] sqlx::Error),
}

/// Check database health by running `SELECT 1`.
///
/// Takes an optional pool reference. Returns `Err(NotConfigured)` if `None`.
/// Uses a 3-second timeout around the query to prevent hanging on a slow
/// or unresponsive database.
pub async fn health_check(pool: Option<&PgPool>) -> Result<(), DbError> {
    let pool = pool.ok_or(DbError::NotConfigured)?;

    tokio::time::timeout(Duration::from_secs(3), async {
        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(DbError::from)
    })
    .await
    .map_err(|_| DbError::PoolTimeout(Duration::from_secs(3)))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_check_none_pool() {
        let result = health_check(None).await;
        assert!(matches!(result, Err(DbError::NotConfigured)));
    }
}
