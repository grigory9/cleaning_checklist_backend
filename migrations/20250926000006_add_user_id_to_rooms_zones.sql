-- Add user_id to existing rooms and zones tables for multi-user support

-- Add user_id to rooms table
ALTER TABLE rooms ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_rooms_user_id ON rooms(user_id);

-- Add user_id to zones table
ALTER TABLE zones ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_zones_user_id ON zones(user_id);