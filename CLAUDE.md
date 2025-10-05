# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building and Running
- **Run the server**: `cargo run` (defaults to port 8080)
- **Build**: `cargo build`
- **Check compilation**: `cargo check`

### Testing
- **Run all tests**: `cargo test`
- **Run specific test**: `cargo test test_name`
- **Run integration tests**: `cargo test --test test_file_name`
- **Run tests with in-memory database**: `DATABASE_URL=sqlite::memory: cargo test`

### Database Setup
- **Create database**: `DATABASE_URL=sqlite:./cleaner.db sqlx database create`
- **Run migrations**: `DATABASE_URL=sqlite:./cleaner.db sqlx migrate run`
- **Reset database**: `rm -f cleaner.db && touch cleaner.db`
- **SQLx prepare (for compile-time checking)**: `DATABASE_URL=sqlite:./cleaner.db cargo sqlx prepare`

### Environment Variables
Required for running:
- `JWT_SECRET`: JWT signing secret (required for authentication)
- `DATABASE_URL`: Database connection string (defaults to `sqlite://./cleaner.db`)

Optional:
- `APP_PORT`: Server port (defaults to 8080)
- `RUST_LOG`: Logging level (defaults to `cleaner_api=info,axum=info,tower_http=info`)

Example run command:
```bash
JWT_SECRET=my-super-secret-jwt-key-for-development DATABASE_URL=sqlite:./cleaner.db cargo run
```

## Architecture Overview

### Core Components
- **Axum Web Framework**: HTTP server and routing
- **SQLx**: Database access with compile-time checked queries
- **SQLite**: Database with migration support
- **OAuth2.0 + JWT**: Authentication system with token refresh

### Authentication Flow
1. **OAuth Client Management**: Admin can create OAuth clients via `/admin/clients`
2. **User Registration/Login**: Direct JWT token issuance (bypassing OAuth authorize flow)
3. **Token Management**: Access tokens (24h) + refresh tokens (30d) with rotation
4. **Authorization**: Bearer token middleware for protected endpoints

### API Structure
- `/api/v1/*`: Main API endpoints (users, rooms, zones, stats)
- `/oauth/*`: OAuth2.0 endpoints (authorize, token, introspect, revoke)
- `/admin/*`: Admin endpoints (OAuth client management)
- `/swagger-ui`: API documentation
- `/api-doc/openapi.json`: OpenAPI specification

### Data Model
- **Users**: Authentication and ownership
- **Rooms**: Top-level containers owned by users
- **Zones**: Cleaning areas within rooms
- **OAuth Clients**: Registered applications
- **Tokens**: Access and refresh token storage

### Module Organization
- `src/api/`: HTTP endpoint handlers
- `src/auth/`: Authentication middleware, password hashing, JWT tokens, OAuth flows
- `src/models/`: Data structures and database schemas
- `src/error/`: Error handling and result types
- `migrations/`: Database schema migrations

### Key Authentication Details
- Client IDs are set by the client-side during registration (security consideration)
- Login endpoint uses hardcoded "ios" client_id (must exist in database)
- JWT tokens include scopes: `rooms:read`, `rooms:write`, `zones:read`, `zones:write`, `stats:read`
- Refresh token rotation is implemented for security

### Testing
- Integration tests use in-memory SQLite (`sqlite::memory:`)
- Tests require `JWT_SECRET` environment variable
- Full flow tests: `tests/register_create_get_room_integration.rs`
- OAuth flow tests: `tests/full_auth_integration.rs`