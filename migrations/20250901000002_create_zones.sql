-- zones
CREATE TABLE IF NOT EXISTS zones (
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
CREATE INDEX IF NOT EXISTS idx_zones_room_id ON zones(room_id);
CREATE INDEX IF NOT EXISTS idx_zones_last_cleaned_at ON zones(last_cleaned_at);
