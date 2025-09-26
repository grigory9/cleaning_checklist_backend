-- Access tokens
CREATE TABLE IF NOT EXISTS access_tokens (
  token_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT, -- NULL for client credentials grant
  scopes TEXT NOT NULL, -- JSON array
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  revoked BOOLEAN DEFAULT FALSE,
  FOREIGN KEY(client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_access_tokens_expires_at ON access_tokens(expires_at);
CREATE INDEX IF NOT EXISTS idx_access_tokens_client_id ON access_tokens(client_id);
CREATE INDEX IF NOT EXISTS idx_access_tokens_user_id ON access_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_access_tokens_revoked ON access_tokens(revoked);