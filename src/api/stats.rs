use axum::{extract::{Query, State}, Json};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use std::sync::Arc;

use crate::{
    error::AppResult,
    models::{compute_is_due, compute_next_due, AppState, Zone, ZoneView},
    api::oauth::AuthenticatedUser,
};

#[derive(Serialize, ToSchema)]
pub struct StatsOverview {
    pub rooms_total: i64,
    pub zones_total: i64,
    pub due_zones: i64,
}

#[utoipa::path(
    get,
    path = "/stats/overview",
    responses(
        (status = 200, description = "Overview stats", body = StatsOverview),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn overview(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> AppResult<Json<StatsOverview>> {
    let (rooms_total,): (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM rooms WHERE user_id = ?1 AND deleted_at IS NULL")
            .bind(&user_id)
            .fetch_one(&state.pool)
            .await?;
    let (zones_total,): (i64,) =
        sqlx::query_as(
            r#"SELECT COUNT(1) FROM zones z
               JOIN rooms r ON z.room_id = r.id
               WHERE r.user_id = ?1 AND z.deleted_at IS NULL"#
        )
        .bind(&user_id)
        .fetch_one(&state.pool)
        .await?;

    let zones: Vec<Zone> = sqlx::query_as(
        r#"SELECT z.id, z.room_id, z.name, z.icon, z.frequency, z.custom_interval_days, z.last_cleaned_at, z.created_at, z.updated_at, z.deleted_at
           FROM zones z
           JOIN rooms r ON z.room_id = r.id
           WHERE r.user_id = ?1 AND z.deleted_at IS NULL"#
    ).bind(&user_id).fetch_all(&state.pool).await?;

    let mut due_zones = 0i64;
    for z in &zones {
        let next_due = compute_next_due(z.last_cleaned_at, &z.frequency, z.custom_interval_days);
        let is_due = compute_is_due(next_due);
        if is_due {
            due_zones += 1;
        }
    }

    Ok(Json(StatsOverview {
        rooms_total,
        zones_total,
        due_zones,
    }))
}

#[derive(Deserialize, IntoParams)]
pub struct DueParams {
    pub within: Option<String>,
}

#[utoipa::path(
    get,
    path = "/zones/due",
    params(DueParams),
    responses(
        (status = 200, description = "Zones due", body = [ZoneView]),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn zones_due(
    State(state): State<Arc<AppState>>,
    Query(p): Query<DueParams>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> AppResult<Json<Vec<ZoneView>>> {
    let within = parse_within(p.within.as_deref()).unwrap_or(Duration::days(7));
    let horizon = Utc::now() + within;

    let zones: Vec<Zone> = sqlx::query_as(
        r#"SELECT z.id, z.room_id, z.name, z.icon, z.frequency, z.custom_interval_days, z.last_cleaned_at, z.created_at, z.updated_at, z.deleted_at
           FROM zones z
           JOIN rooms r ON z.room_id = r.id
           WHERE r.user_id = ?1 AND z.deleted_at IS NULL"#
    ).bind(&user_id).fetch_all(&state.pool).await?;

    let mut out = Vec::new();
    for z in zones {
        let next_due = compute_next_due(z.last_cleaned_at, &z.frequency, z.custom_interval_days);
        let is_due = match next_due {
            Some(dt) => dt <= horizon,
            None => true,
        };
        if is_due {
            out.push(ZoneView {
                id: z.id,
                room_id: z.room_id,
                name: z.name,
                icon: z.icon,
                frequency: z.frequency,
                custom_interval_days: z.custom_interval_days,
                last_cleaned_at: z.last_cleaned_at,
                next_due_at: next_due,
                is_due: true,
                created_at: z.created_at,
                updated_at: z.updated_at,
                deleted_at: z.deleted_at,
            });
        }
    }

    Ok(Json(out))
}

fn parse_within(s: Option<&str>) -> Option<Duration> {
    let s = s?;
    let s = s.trim();
    if s.ends_with("d") {
        s[..s.len() - 1].parse::<i64>().ok().map(Duration::days)
    } else if s.ends_with("h") {
        s[..s.len() - 1].parse::<i64>().ok().map(Duration::hours)
    } else if s.ends_with("w") {
        s[..s.len() - 1]
            .parse::<i64>()
            .ok()
            .map(|w| Duration::days(w * 7))
    } else {
        None
    }
}

use axum::{
    routing::get,
    Router,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/overview", get(overview))
        .route("/zones/due", get(zones_due))
}
