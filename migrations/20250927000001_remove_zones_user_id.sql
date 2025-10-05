-- Remove redundant user_id from zones table
-- Zones inherit user ownership through the room relationship

-- Drop the index first
DROP INDEX IF EXISTS idx_zones_user_id;

-- Remove the user_id column from zones table
-- Note: SQLite doesn't support DROP COLUMN directly, so we need to recreate the table
PRAGMA foreign_keys=off;

-- Create new table without user_id
CREATE TABLE zones_new (
  id TEXT PRIMARY KEY,
  room_id TEXT NOT NULL,
  name TEXT NOT NULL,
  icon TEXT,
  frequency TEXT NOT NULL,
  custom_interval_days INTEGER,
  last_cleaned_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  FOREIGN KEY(room_id) REFERENCES rooms(id)
);

-- Copy data from old table (excluding user_id)
INSERT INTO zones_new (id, room_id, name, icon, frequency, custom_interval_days, last_cleaned_at, created_at, updated_at, deleted_at)
SELECT id, room_id, name, icon, frequency, custom_interval_days, last_cleaned_at, created_at, updated_at, deleted_at
FROM zones;

-- Drop old table and rename new one
DROP TABLE zones;
ALTER TABLE zones_new RENAME TO zones;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_zones_room_id ON zones(room_id);
CREATE INDEX IF NOT EXISTS idx_zones_last_cleaned_at ON zones(last_cleaned_at);

PRAGMA foreign_keys=on;