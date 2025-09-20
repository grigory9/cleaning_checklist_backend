-- Add user_id column to rooms table for multi-tenant support
ALTER TABLE rooms ADD COLUMN user_id TEXT NOT NULL DEFAULT '';

-- Create index on user_id for performance
CREATE INDEX IF NOT EXISTS idx_rooms_user_id ON rooms(user_id);