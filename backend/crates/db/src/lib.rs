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
/// Uses a timeout around the query to prevent hanging on a slow or unresponsive database.
pub async fn health_check(pool: Option<&PgPool>) -> Result<(), DbError> {
    const QUERY_TIMEOUT: Duration = Duration::from_secs(3);
    let pool = pool.ok_or(DbError::NotConfigured)?;

    tokio::time::timeout(QUERY_TIMEOUT, async {
        sqlx::query("SELECT 1")
            .fetch_one(pool)
            .await
            .map(|_| ())
            .map_err(DbError::from)
    })
    .await
    .map_err(|_| DbError::PoolTimeout(QUERY_TIMEOUT))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_check_none_pool() {
        let result = health_check(None).await;
        assert!(matches!(result, Err(DbError::NotConfigured)));
    }

    #[test]
    fn error_display_renders_variants() {
        let nc = DbError::NotConfigured;
        assert_eq!(nc.to_string(), "database not configured");

        let pt = DbError::PoolTimeout(Duration::from_secs(3));
        assert!(pt.to_string().contains("3s"), "expected '3s' in: {}", pt);

        // QueryFailed wraps sqlx::Error — just verify it's not empty
        let qf = DbError::QueryFailed(sqlx::Error::PoolTimedOut);
        assert!(!qf.to_string().is_empty());
    }
}
