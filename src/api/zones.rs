use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use utoipa::{IntoParams, ToSchema};

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::{compute_is_due, compute_next_due, AppState, NewZone, UpdateZone, Zone, ZoneView},
    api::oauth::AuthenticatedUser,
};

#[derive(Deserialize, IntoParams)]
pub struct ListZones {
    pub only_due: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/rooms/{room_id}/zones",
    params(("room_id" = String, Path, description = "Room id"), ListZones),
    responses(
        (status = 200, description = "List zones", body = [ZoneView]),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn list_zones(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Query(p): Query<ListZones>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> AppResult<Json<Vec<ZoneView>>> {
    // Проверяем, что комната принадлежит пользователю
    let room_exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(1) FROM rooms WHERE id = ?1 AND user_id = ?2 AND deleted_at IS NULL"
    )
    .bind(&room_id)
    .bind(&user_id)
    .fetch_one(&state.pool)
    .await?;
    
    if room_exists.0 == 0 {
        return Err(AppError::NotFound);
    }

    let mut zones: Vec<Zone> = sqlx::query_as::<_, Zone>(
        r#"SELECT id, room_id, name, icon, frequency, custom_interval_days, last_cleaned_at, created_at, updated_at, deleted_at
           FROM zones WHERE room_id = ?1 AND deleted_at IS NULL
           ORDER BY created_at DESC"#
    ).bind(&room_id).fetch_all(&state.pool).await?;

    let mut out = Vec::with_capacity(zones.len());
    for z in zones.drain(..) {
        let next_due = compute_next_due(z.last_cleaned_at, &z.frequency, z.custom_interval_days);
        let is_due = compute_is_due(next_due);
        if p.only_due.unwrap_or(false) && !is_due {
            continue;
        }
        out.push(ZoneView {
            id: z.id,
            room_id: z.room_id,
            name: z.name,
            icon: z.icon,
            frequency: z.frequency,
            custom_interval_days: z.custom_interval_days,
            last_cleaned_at: z.last_cleaned_at,
            next_due_at: next_due,
            is_due,
            created_at: z.created_at,
            updated_at: z.updated_at,
            deleted_at: z.deleted_at,
        });
    }
    Ok(Json(out))
}

#[utoipa::path(
    post,
    path = "/rooms/{room_id}/zones",
    params(("room_id" = String, Path, description = "Room id")),
    request_body = NewZone,
    responses(
        (status = 201, description = "Zone created", body = ZoneView),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn create_zone(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    Path(room_id): Path<String>,
    Json(body): Json<NewZone>,
) -> AppResult<(axum::http::StatusCode, Json<ZoneView>)> {
    if body.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    if matches!(body.frequency, crate::models::Frequency::Custom)
        && body.custom_interval_days.unwrap_or(0) == 0
    {
        return Err(AppError::Validation(
            "custom_interval_days must be >= 1 for custom frequency".into(),
        ));
    }
    // проверим, что комната существует, не удалена и принадлежит пользователю
    let exists: (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM rooms WHERE id = ?1 AND user_id = ?2 AND deleted_at IS NULL")
            .bind(&room_id)
            .bind(&user_id)
            .fetch_one(&state.pool)
            .await?;
    if exists.0 == 0 {
        return Err(AppError::NotFound);
    }

    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let name = body.name;
    let icon = body.icon;
    let frequency = body.frequency.as_str().to_string();
    let custom_interval_days = body.custom_interval_days.map(|v| v as i64);
    sqlx::query(
        r#"INSERT INTO zones(id, room_id, name, icon, frequency, custom_interval_days, last_cleaned_at, created_at, updated_at, deleted_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7, NULL)"#,
    )
    .bind(&id)
    .bind(&room_id)
    .bind(&name)
    .bind(&icon)
    .bind(&frequency)
    .bind(custom_interval_days)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let next_due = None;
    let is_due = true; // ещё не убиралось
    let view = ZoneView {
        id,
        room_id,
        name,
        icon,
        frequency,
        custom_interval_days,
        last_cleaned_at: None,
        next_due_at: next_due,
        is_due,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    Ok((axum::http::StatusCode::CREATED, Json(view)))
}

#[utoipa::path(
    get,
    path = "/zones/{id}",
    params(("id" = String, Path, description = "Zone id")),
    responses(
        (status = 200, description = "Zone details", body = ZoneView),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn get_zone(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> AppResult<Json<ZoneView>> {
    let z = sqlx::query_as::<_, Zone>(
        r#"SELECT z.id, z.room_id, z.name, z.icon, z.frequency, z.custom_interval_days, z.last_cleaned_at, z.created_at, z.updated_at, z.deleted_at
           FROM zones z
           JOIN rooms r ON z.room_id = r.id
           WHERE z.id = ?1 AND z.deleted_at IS NULL AND r.user_id = ?2 AND r.deleted_at IS NULL"#
    ).bind(&id).bind(&user_id).fetch_optional(&state.pool).await?;
    let z = z.ok_or(AppError::NotFound)?;
    let next_due = compute_next_due(z.last_cleaned_at, &z.frequency, z.custom_interval_days);
    let is_due = compute_is_due(next_due);
    Ok(Json(ZoneView {
        id: z.id,
        room_id: z.room_id,
        name: z.name,
        icon: z.icon,
        frequency: z.frequency,
        custom_interval_days: z.custom_interval_days,
        last_cleaned_at: z.last_cleaned_at,
        next_due_at: next_due,
        is_due,
        created_at: z.created_at,
        updated_at: z.updated_at,
        deleted_at: z.deleted_at,
    }))
}

#[utoipa::path(
    patch,
    path = "/zones/{id}",
    params(("id" = String, Path, description = "Zone id")),
    request_body = UpdateZone,
    responses(
        (status = 200, description = "Zone updated", body = ZoneView),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn update_zone(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateZone>,
) -> AppResult<Json<ZoneView>> {
    let z = sqlx::query_as::<_, Zone>(
        r#"SELECT z.id, z.room_id, z.name, z.icon, z.frequency, z.custom_interval_days, z.last_cleaned_at, z.created_at, z.updated_at, z.deleted_at
           FROM zones z
           JOIN rooms r ON z.room_id = r.id
           WHERE z.id = ?1 AND z.deleted_at IS NULL AND r.user_id = ?2 AND r.deleted_at IS NULL"#
    ).bind(&id).bind(&user_id).fetch_optional(&state.pool).await?;
    let mut z = z.ok_or(AppError::NotFound)?;

    let now = Utc::now();
    let name = body.name.unwrap_or(z.name.clone());
    let icon = body.icon.or(z.icon.clone());
    let frequency = body
        .frequency
        .map(|f| f.as_str().to_string())
        .unwrap_or(z.frequency.clone());
    let custom_interval_days = body
        .custom_interval_days
        .map(|v| v as i64)
        .or(z.custom_interval_days);

    if frequency == "custom" && custom_interval_days.unwrap_or(0) <= 0 {
        return Err(AppError::Validation(
            "custom_interval_days must be >= 1".into(),
        ));
    }

    sqlx::query(
        "UPDATE zones SET name = ?1, icon = ?2, frequency = ?3, custom_interval_days = ?4, updated_at = ?5 WHERE id = ?6",
    )
    .bind(&name)
    .bind(&icon)
    .bind(&frequency)
    .bind(custom_interval_days)
    .bind(now)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    z.name = name.clone();
    z.icon = icon.clone();
    z.frequency = frequency.clone();
    z.custom_interval_days = custom_interval_days;
    z.updated_at = now;
    let next_due = compute_next_due(z.last_cleaned_at, &z.frequency, z.custom_interval_days);
    let is_due = compute_is_due(next_due);
    Ok(Json(ZoneView {
        id: z.id,
        room_id: z.room_id,
        name: z.name,
        icon: z.icon,
        frequency: z.frequency,
        custom_interval_days: z.custom_interval_days,
        last_cleaned_at: z.last_cleaned_at,
        next_due_at: next_due,
        is_due,
        created_at: z.created_at,
        updated_at: z.updated_at,
        deleted_at: z.deleted_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/zones/{id}",
    params(("id" = String, Path, description = "Zone id")),
    responses(
        (status = 204, description = "Zone deleted"),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn delete_zone(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> AppResult<axum::http::StatusCode> {
    let now = Utc::now();
    let res = sqlx::query(
        r#"UPDATE zones SET deleted_at = ?1
           WHERE id = ?2 AND deleted_at IS NULL
           AND room_id IN (SELECT id FROM rooms WHERE user_id = ?3 AND deleted_at IS NULL)"#,
    )
    .bind(now)
    .bind(&id)
    .bind(&user_id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct CleanBody {
    pub cleaned_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[utoipa::path(
    post,
    path = "/zones/{id}/clean",
    params(("id" = String, Path, description = "Zone id")),
    request_body = CleanBody,
    responses(
        (status = 200, description = "Zone cleaned", body = ZoneView),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn clean_zone(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    Path(id): Path<String>,
    Json(body): Json<CleanBody>,
) -> AppResult<Json<ZoneView>> {
    let cleaned_at = body.cleaned_at.unwrap_or_else(chrono::Utc::now);
    let res = sqlx::query(
        r#"UPDATE zones SET last_cleaned_at = ?1, updated_at = ?1
           WHERE id = ?2 AND deleted_at IS NULL
           AND room_id IN (SELECT id FROM rooms WHERE user_id = ?3 AND deleted_at IS NULL)"#
    )
        .bind(cleaned_at)
        .bind(&id)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    get_zone(State(state), Path(id), AuthenticatedUser(user_id)).await
}

#[derive(Deserialize, ToSchema)]
pub struct BulkClean {
    pub zone_ids: Vec<String>,
    pub cleaned_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct BulkCleanResponse {
    pub updated: u64,
}

#[utoipa::path(
    post,
    path = "/zones/bulk/clean",
    request_body = BulkClean,
    responses(
        (status = 200, description = "Bulk clean result", body = BulkCleanResponse),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn bulk_clean(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    Json(body): Json<BulkClean>,
) -> AppResult<Json<BulkCleanResponse>> {
    let cleaned_at = body.cleaned_at.unwrap_or_else(chrono::Utc::now);
    let mut updated = 0u64;
    for id in body.zone_ids.iter() {
        let res = sqlx::query(
            r#"UPDATE zones SET last_cleaned_at = ?1, updated_at = ?1
               WHERE id = ?2 AND deleted_at IS NULL
               AND room_id IN (SELECT id FROM rooms WHERE user_id = ?3 AND deleted_at IS NULL)"#
        )
            .bind(cleaned_at)
            .bind(id)
            .bind(&user_id)
            .execute(&state.pool)
            .await?;
        updated += res.rows_affected();
    }
    Ok(Json(BulkCleanResponse { updated }))
}

use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/rooms/:room_id/zones", get(list_zones).post(create_zone))
        .route("/zones/:id", get(get_zone).patch(update_zone).delete(delete_zone))
        .route("/zones/:id/clean", post(clean_zone))
        .route("/zones/bulk/clean", post(bulk_clean))
}
