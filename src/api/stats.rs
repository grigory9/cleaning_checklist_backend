use axum::{extract::Query, Json};
use chrono::{Duration, Utc};
use serde::Deserialize;

use crate::{
    error::AppResult,
    models::{compute_is_due, compute_next_due, AppState, Zone, ZoneView},
};

#[utoipa::path(get, path="/api/v1/stats/overview", tag="stats",
    responses((status=200, body=serde_json::Value))
)]
pub async fn overview(
    state: axum::extract::State<std::sync::Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let (rooms_total,): (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM rooms WHERE deleted_at IS NULL")
            .fetch_one(&state.pool)
            .await?;
    let (zones_total,): (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM zones WHERE deleted_at IS NULL")
            .fetch_one(&state.pool)
            .await?;

    let zones: Vec<Zone> = sqlx::query_as(
        r#"SELECT id, room_id, name, icon, frequency, custom_interval_days, last_cleaned_at, created_at, updated_at, deleted_at
           FROM zones WHERE deleted_at IS NULL"#
    ).fetch_all(&state.pool).await?;

    let mut due_zones = 0i64;
    for z in &zones {
        let next_due = compute_next_due(z.last_cleaned_at, &z.frequency, z.custom_interval_days);
        let is_due = compute_is_due(next_due);
        if is_due {
            due_zones += 1;
        }
    }

    Ok(Json(serde_json::json!({
        "rooms_total": rooms_total,
        "zones_total": zones_total,
        "due_zones": due_zones,
    })))
}

#[derive(Deserialize)]
pub struct DueParams {
    pub within: Option<String>,
}

#[utoipa::path(get, path="/api/v1/zones/due", tag="stats",
    params(DueParams),
    responses((status=200, body=Vec<ZoneView>))
)]
pub async fn zones_due(
    state: axum::extract::State<std::sync::Arc<AppState>>,
    Query(p): Query<DueParams>,
) -> AppResult<Json<Vec<ZoneView>>> {
    let within = parse_within(p.within.as_deref()).unwrap_or(Duration::days(7));
    let horizon = Utc::now() + within;

    let zones: Vec<Zone> = sqlx::query_as(
        r#"SELECT id, room_id, name, icon, frequency, custom_interval_days, last_cleaned_at, created_at, updated_at, deleted_at
           FROM zones WHERE deleted_at IS NULL"#
    ).fetch_all(&state.pool).await?;

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
