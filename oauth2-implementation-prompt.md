# Claude Code Prompt: Implement OAuth2.0 Authorization Server for Cleaning Checklist Backend

## Context
I have a Rust-based cleaning checklist backend API built with Axum and SQLite. Currently, it's single-user with no authentication. I need to convert it to a multi-user system by implementing OAuth2.0 protocol where my backend acts as the authorization server.

## Current Architecture
- **Framework**: Axum (Rust)
- **Database**: SQLite with SQLx
- **API Documentation**: OpenAPI 3.0 with utoipa
- **Models**: Rooms and Zones for cleaning management
- **Features**: Soft deletion, frequency tracking, statistics

## Requirements

### 1. OAuth2.0 Authorization Server Implementation
Implement my backend as an OAuth2.0 provider supporting:
- **Authorization Code Grant** - for web/mobile apps
- **Client Credentials Grant** - for server-to-server
- **Refresh Token Grant** - for token renewal
- **Optional**: Password Grant (though deprecated in OAuth 2.1)

### 2. Database Schema
Create new tables:
```sql
-- Users table
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  username TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  name TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  email_verified BOOLEAN DEFAULT FALSE
);

-- OAuth clients (applications that can access the API)
CREATE TABLE oauth_clients (
  client_id TEXT PRIMARY KEY,
  client_secret_hash TEXT NOT NULL,
  name TEXT NOT NULL,
  redirect_uris TEXT NOT NULL, -- JSON array of allowed URIs
  grant_types TEXT NOT NULL, -- JSON array of allowed grant types
  scopes TEXT NOT NULL, -- JSON array of allowed scopes
  created_at TEXT NOT NULL
);

-- Authorization codes (temporary codes exchanged for tokens)
CREATE TABLE authorization_codes (
  code TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  redirect_uri TEXT NOT NULL,
  scopes TEXT NOT NULL, -- JSON array
  expires_at TEXT NOT NULL,
  code_challenge TEXT, -- for PKCE
  code_challenge_method TEXT, -- for PKCE
  FOREIGN KEY(client_id) REFERENCES oauth_clients(client_id),
  FOREIGN KEY(user_id) REFERENCES users(id)
);

-- Access tokens
CREATE TABLE access_tokens (
  token_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT, -- NULL for client credentials grant
  scopes TEXT NOT NULL, -- JSON array
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(client_id) REFERENCES oauth_clients(client_id),
  FOREIGN KEY(user_id) REFERENCES users(id)
);

-- Refresh tokens
CREATE TABLE refresh_tokens (
  token_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT,
  scopes TEXT NOT NULL, -- JSON array
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(client_id) REFERENCES oauth_clients(client_id),
  FOREIGN KEY(user_id) REFERENCES users(id)
);
```

Add `user_id` to existing `rooms` and `zones` tables.

### 3. OAuth2.0 Endpoints
Implement standard OAuth2.0 endpoints:
```
/oauth/
├── GET  /authorize      - Authorization endpoint (show login/consent page)
├── POST /authorize      - Handle user login and consent
├── POST /token          - Token endpoint (exchange code for tokens)
├── POST /revoke         - Revoke tokens
├── GET  /introspect     - Token introspection (validate tokens)
└── GET  /.well-known/oauth-authorization-server - Discovery endpoint

/api/v1/
├── POST /register       - User registration
├── POST /login          - Direct login (returns session, not OAuth tokens)
├── POST /logout         - Logout user
├── GET  /me             - Get current user info (requires valid token)
```

### 4. OAuth2.0 Flows Implementation

#### Authorization Code Flow:
1. Client redirects user to `/oauth/authorize`
2. User logs in and approves access
3. Server redirects back with authorization code
4. Client exchanges code for tokens at `/oauth/token`

#### Client Credentials Flow:
1. Client sends credentials directly to `/oauth/token`
2. Server returns access token (no refresh token)

### 5. Security Features
- **PKCE** (Proof Key for Code Exchange) support for public clients
- **Token introspection** for resource server validation
- **Scope-based access control** (e.g., `read:rooms`, `write:zones`, `admin`)
- **Rate limiting** on token endpoints
- **Secure token generation** (cryptographically random)
- **Password hashing** with Argon2 or bcrypt
- **CSRF protection** for authorization endpoint

### 6. Token Management
- **Access tokens**: Short-lived (15-60 minutes)
- **Refresh tokens**: Long-lived (7-30 days)
- **Authorization codes**: Very short-lived (10 minutes max)
- Implement token rotation on refresh
- Store only hashed versions of tokens in database

### 7. Middleware Updates
Create middleware to:
- Extract and validate bearer tokens from Authorization header
- Check token scopes against required endpoint permissions
- Inject user context into request handlers
- Handle both session-based auth (for web UI) and token-based auth (for API)

### 8. Scopes Design
Define scopes for fine-grained access control:
```
- rooms:read    - View rooms
- rooms:write   - Create/update/delete rooms
- zones:read    - View zones
- zones:write   - Create/update/delete zones
- stats:read    - View statistics
- user:read     - Read user profile
- user:write    - Update user profile
- admin         - Full administrative access
```

### 9. Client Management
- CLI or simple admin endpoint to register OAuth clients
- Generate secure client_id and client_secret
- Manage allowed redirect URIs and grant types per client

### 10. Dependencies to Add
Suggest Rust crates for:
- OAuth2.0 server implementation (or build from scratch)
- JWT generation and validation
- Password hashing (Argon2)
- CSRF token generation
- Secure random token generation

### 11. Testing Strategy
- Unit tests for token generation/validation
- Integration tests for complete OAuth flows
- Test PKCE implementation
- Test scope enforcement
- Mock OAuth client for testing

## Code Structure
```
src/
├── auth/
│   ├── mod.rs
│   ├── oauth/
│   │   ├── mod.rs
│   │   ├── authorize.rs    - Authorization endpoint
│   │   ├── token.rs         - Token endpoint
│   │   ├── introspect.rs    - Token introspection
│   │   └── revoke.rs        - Token revocation
│   ├── middleware.rs        - Auth middleware
│   ├── password.rs          - Password hashing
│   ├── tokens.rs            - Token generation/validation
│   └── scopes.rs            - Scope management
├── models/
│   ├── user.rs
│   ├── oauth_client.rs
│   └── token.rs
```

## Migration Strategy
1. Implement user registration and basic auth first
2. Add OAuth2.0 authorization server endpoints
3. Update existing endpoints with authentication middleware
4. Create migration tool for any existing data
5. Provide OAuth client SDK examples

## Deliverables
1. Complete OAuth2.0 authorization server implementation
2. Database migrations
3. Updated API with authentication
4. Admin tools for client management
5. Documentation for OAuth flow
6. Example client implementations (curl examples)
7. Security considerations documentation

Please implement this step-by-step, starting with the user management and basic authentication, then building up to the full OAuth2.0 server. Explain the security decisions and trade-offs made.