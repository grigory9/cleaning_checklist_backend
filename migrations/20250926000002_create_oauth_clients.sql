-- OAuth clients (applications that can access the API)
CREATE TABLE IF NOT EXISTS oauth_clients (
  client_id TEXT PRIMARY KEY,
  client_secret_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  redirect_uris TEXT NOT NULL, -- JSON array of allowed URIs
  grant_types TEXT NOT NULL, -- JSON array of allowed grant types
  scopes TEXT NOT NULL, -- JSON array of allowed scopes
  created_at TEXT NOT NULL,
  is_public BOOLEAN DEFAULT FALSE -- for PKCE clients
);

CREATE INDEX IF NOT EXISTS idx_oauth_clients_created_at ON oauth_clients(created_at);