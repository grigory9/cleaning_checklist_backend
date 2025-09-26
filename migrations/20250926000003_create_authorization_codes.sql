-- Authorization codes (temporary codes exchanged for tokens)
CREATE TABLE IF NOT EXISTS authorization_codes (
  code TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  redirect_uri TEXT NOT NULL,
  scopes TEXT NOT NULL, -- JSON array
  expires_at TEXT NOT NULL,
  code_challenge TEXT, -- for PKCE
  code_challenge_method TEXT, -- for PKCE
  created_at TEXT NOT NULL,
  FOREIGN KEY(client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_authorization_codes_expires_at ON authorization_codes(expires_at);
CREATE INDEX IF NOT EXISTS idx_authorization_codes_client_id ON authorization_codes(client_id);
CREATE INDEX IF NOT EXISTS idx_authorization_codes_user_id ON authorization_codes(user_id);