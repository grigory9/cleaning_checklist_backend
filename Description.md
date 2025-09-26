# Cleaning Checklist Backend API

## Purpose

A REST API backend for a cleaning checklist application that helps users manage cleaning tasks across different rooms and zones with customizable schedules and frequency tracking.

## Architecture

### Technology Stack
- **Language**: Rust
- **Framework**: Axum (async web framework)
- **Database**: SQLite with SQLx for type-safe database operations
- **API Documentation**: OpenAPI 3.0 with Swagger UI (utoipa)
- **Authentication**: Currently none (planned for OAuth2 branch)

### Core Models

#### Rooms
- Represent physical spaces (e.g., Kitchen, Bathroom, Living Room)
- Have unique IDs, names, and optional icons
- Support soft deletion with `deleted_at` timestamps
- Can be restored after deletion

#### Zones
- Specific cleaning areas within rooms (e.g., "Kitchen Counter", "Bathroom Mirror")
- Linked to parent rooms via foreign key relationship
- Support multiple cleaning frequencies:
  - Daily, Weekly, Monthly
  - Custom intervals (configurable days)
- Track last cleaned timestamp and automatically calculate due dates
- Support soft deletion

### API Structure

```
/api/v1/
├── rooms/
│   ├── GET    /           - List rooms (with optional stats)
│   ├── POST   /           - Create new room
│   ├── GET    /:id        - Get specific room
│   ├── PATCH  /:id        - Update room
│   ├── DELETE /:id        - Soft delete room
│   └── POST   /:id/restore - Restore deleted room
├── rooms/:room_id/zones/
│   ├── GET    /           - List zones in room
│   └── POST   /           - Create new zone in room
├── zones/
│   ├── GET    /:id        - Get specific zone
│   ├── PATCH  /:id        - Update zone
│   ├── DELETE /:id        - Soft delete zone
│   ├── POST   /:id/clean  - Mark zone as cleaned
│   ├── POST   /bulk/clean - Mark multiple zones as cleaned
│   └── GET    /due        - Get zones that are due for cleaning
└── stats/
    └── GET    /overview   - Get cleaning statistics overview
```

### Key Features

1. **Frequency Management**: Flexible cleaning schedules with automatic due date calculation
2. **Soft Deletion**: Resources are marked as deleted rather than physically removed
3. **Timezone Support**: Uses UTC timestamps with chrono-tz for timezone handling
4. **Statistics**: Room and zone cleaning statistics with completion tracking
5. **Search**: Text-based search functionality for rooms
6. **Bulk Operations**: Batch cleaning operations for multiple zones
7. **OpenAPI Documentation**: Auto-generated API documentation with Swagger UI

### Database Schema

#### Rooms Table
```sql
CREATE TABLE rooms (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  icon TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);
```

#### Zones Table
```sql
CREATE TABLE zones (
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
```

### Development Features

- **Migrations**: Database schema versioning with SQLx migrations
- **Environment Configuration**: Configurable via environment variables
- **Logging**: Structured logging with tracing
- **CORS Support**: Cross-origin resource sharing enabled
- **Error Handling**: Centralized error handling with custom error types

### Endpoints Access

- **API Server**: http://localhost:8080
- **OpenAPI JSON**: http://localhost:8080/api-doc/openapi.json
- **Swagger UI**: http://localhost:8080/swagger-ui

This backend is designed to be consumed by a frontend application for managing household or commercial cleaning schedules with detailed tracking and reporting capabilities.