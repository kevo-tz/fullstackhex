-- Add composite index for paginated notes queries
-- The list_notes endpoint uses WHERE user_id = $1 ORDER BY created_at DESC LIMIT/OFFSET
-- The existing idx_notes_user_id covers the WHERE clause but requires in-memory sort
CREATE INDEX IF NOT EXISTS idx_notes_user_created ON notes(user_id, created_at DESC);
