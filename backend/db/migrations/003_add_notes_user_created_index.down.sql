-- Remove composite index for paginated notes queries
DROP INDEX IF EXISTS idx_notes_user_created;
