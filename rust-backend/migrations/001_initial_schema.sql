-- Initial schema setup
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    execution_time BIGINT NOT NULL,
    installed_by TEXT NOT NULL
);

-- Example health check table
CREATE TABLE IF NOT EXISTS health_status (
    id SERIAL PRIMARY KEY,
    service_name VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'healthy',
    last_checked TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service_name)
);

INSERT INTO health_status (service_name, status)
VALUES ('bare-metal-rust', 'healthy')
ON CONFLICT (service_name) DO UPDATE SET last_checked = CURRENT_TIMESTAMP;
