-- Create schemas for service isolation
CREATE SCHEMA IF NOT EXISTS rust_service;
CREATE SCHEMA IF NOT EXISTS python_service;

-- Set default search path
ALTER DATABASE app_database SET search_path TO rust_service, python_service, public;

-- Grant privileges
GRANT ALL ON SCHEMA rust_service TO app_user;
GRANT ALL ON SCHEMA python_service TO app_user;
