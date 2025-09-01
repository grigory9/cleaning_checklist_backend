use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{AppState, NewRoom, Room, RoomView, UpdateRoom},
};

#[derive(Deserialize)]
pub struct ListParams {
    pub with_stats: Option<bool>,
    pub q: Option<String>,
}

#[utoipa::path(
    get, path="/api/v1/rooms", tag="rooms",
    params(ListParams),
    responses((status=200, description="OK", body=Vec<RoomView>))
)]
pub async fn list_rooms(
    State(state): State<std::sync::Arc<AppState>>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<Vec<RoomView>>> {
    let mut rooms: Vec<Room> = sqlx::query_as::<_, Room>(
        r#"SELECT id, name, icon, created_at, updated_at, deleted_at
           FROM rooms
           WHERE deleted_at IS NULL AND (?1 IS NULL OR name LIKE '%' || ?1 || '%')
           ORDER BY created_at DESC"#,
    )
    .bind(p.q)
    .fetch_all(&state.pool)
    .await?;

    let with_stats = p.with_stats.unwrap_or(false);
    let mut out: Vec<RoomView> = Vec::with_capacity(rooms.len());
    for r in rooms.drain(..) {
        if with_stats {
            let rec = sqlx::query!(
                r#"SELECT COUNT(*) as zones_total,
                          MAX(last_cleaned_at) as last_cleaned_at
                   FROM zones WHERE room_id = ?1 AND deleted_at IS NULL"#,
                r.id
            )
            .fetch_one(&state.pool)
            .await?;
            let zones_total = rec.zones_total.unwrap_or(0);
            // Упрощённо: считаем "очищено" как next_due ещё не наступил
            let cleaned_count = sqlx::query(sql)!(
                r#"SELECT COUNT(*) as cnt
                   FROM zones
                   WHERE room_id = ?1 AND deleted_at IS NULL
                     AND (last_cleaned_at IS NOT NULL)"#,
                r.id
            )
            .fetch_one(&state.pool)
            .await?
            .cnt
            .unwrap_or(0);

            out.push(RoomView {
                id: r.id,
                name: r.name,
                icon: r.icon,
                created_at: r.created_at,
                updated_at: r.updated_at,
                deleted_at: r.deleted_at,
                zones_total: Some(zones_total),
                zones_cleaned_count: Some(cleaned_count),
                last_cleaned_at: rec.last_cleaned_at,
            });
        } else {
            out.push(RoomView {
                id: r.id,
                name: r.name,
                icon: r.icon,
                created_at: r.created_at,
                updated_at: r.updated_at,
                deleted_at: r.deleted_at,
                zones_total: None,
                zones_cleaned_count: None,
                last_cleaned_at: None,
            });
        }
    }
    Ok(Json(out))
}

#[utoipa::path(post,
    path="/api/v1/rooms",
    tag="rooms",
    request_body=NewRoom,
    responses((status=201, description="Created", body=RoomView))
)]
pub async fn create_room(
    State(state): State<std::sync::Arc<AppState>>,
    Json(body): Json<NewRoom>,
) -> AppResult<(axum::http::StatusCode, Json<RoomView>)> {
    if body.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO rooms(id, name, icon, created_at, updated_at, deleted_at)
           VALUES (?1, ?2, ?3, ?4, ?5, NULL)"#,
        id,
        body.name,
        body.icon,
        now,
        now
    )
    .execute(&state.pool)
    .await?;

    let view = RoomView {
        id,
        name: body.name,
        icon: body.icon,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        zones_total: Some(0),
        zones_cleaned_count: Some(0),
        last_cleaned_at: None,
    };
    Ok((axum::http::StatusCode::CREATED, Json(view)))
}

#[utoipa::path(get, path="/api/v1/rooms/{id}", tag="rooms",
    params(("id" = String, Path, description = "Room id")),
    responses((status=200, body=RoomView), (status=404))
)]
pub async fn get_room(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<RoomView>> {
    let r = sqlx::query_as::<_, Room>(
        r#"SELECT id, name, icon, created_at, updated_at, deleted_at
           FROM rooms WHERE id = ?1 AND deleted_at IS NULL"#,
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?;
    let r = r.ok_or(AppError::NotFound)?;
    Ok(Json(RoomView {
        id: r.id,
        name: r.name,
        icon: r.icon,
        created_at: r.created_at,
        updated_at: r.updated_at,
        deleted_at: r.deleted_at,
        zones_total: None,
        zones_cleaned_count: None,
        last_cleaned_at: None,
    }))
}

#[utoipa::path(patch, path="/api/v1/rooms/{id}", tag="rooms",
    request_body=UpdateRoom,
    params(("id" = String, Path)),
    responses((status=200, body=RoomView), (status=404))
)]
pub async fn update_room(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRoom>,
) -> AppResult<Json<RoomView>> {
    let now = Utc::now();
    let rec = sqlx::query_as::<_, Room>(
        "SELECT id, name, icon, created_at, updated_at, deleted_at FROM rooms WHERE id = ?1 AND deleted_at IS NULL"
    ).bind(&id).fetch_optional(&state.pool).await?;
    let mut r = rec.ok_or(AppError::NotFound)?;

    let name = body.name.unwrap_or(r.name.clone());
    let icon = body.icon.or(r.icon.clone());

    sqlx::query!(
        "UPDATE rooms SET name = ?1, icon = ?2, updated_at = ?3 WHERE id = ?4",
        name,
        icon,
        now,
        id
    )
    .execute(&state.pool)
    .await?;

    r.name = name;
    r.icon = icon;
    r.updated_at = now;
    Ok(Json(RoomView {
        id: r.id,
        name: r.name,
        icon: r.icon,
        created_at: r.created_at,
        updated_at: r.updated_at,
        deleted_at: r.deleted_at,
        zones_total: None,
        zones_cleaned_count: None,
        last_cleaned_at: None,
    }))
}

#[utoipa::path(delete, path="/api/v1/rooms/{id}", tag="rooms",
    params(("id" = String, Path)),
    responses((status=204), (status=404))
)]
pub async fn delete_room(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    let now = Utc::now();
    let res = sqlx::query!(
        "UPDATE rooms SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        now,
        id
    )
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    // мягко скрываем зоны
    sqlx::query!(
        "UPDATE zones SET deleted_at = ?1 WHERE room_id = ?2 AND deleted_at IS NULL",
        now,
        id
    )
    .execute(&state.pool)
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path="/api/v1/rooms/{id}/restore", tag="rooms",
    params(("id" = String, Pat)),
    responses((status=200, body=RoomView), (status=404))
)]
pub async fn restore_room(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<RoomView>> {
    let res = sqlx::query!("UPDATE rooms SET deleted_at = NULL WHERE id = ?1", id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let r = sqlx::query_as::<_, Room>(
        "SELECT id, name, icon, created_at, updated_at, deleted_at FROM rooms WHERE id = ?1",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(RoomView {
        id: r.id,
        name: r.name,
        icon: r.icon,
        created_at: r.created_at,
        updated_at: r.updated_at,
        deleted_at: r.deleted_at,
        zones_total: None,
        zones_cleaned_count: None,
        last_cleaned_at: None,
    }))
}
