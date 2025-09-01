use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use utoipa::ToSchema;
use uuid::Uuid;

pub type Db = SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: Db,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Custom,
}

impl Frequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Frequency::Daily => "daily",
            Frequency::Weekly => "weekly",
            Frequency::Monthly => "monthly",
            Frequency::Custom => "custom",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "daily" => Some(Frequency::Daily),
            "weekly" => Some(Frequency::Weekly),
            "monthly" => Some(Frequency::Monthly),
            "custom" => Some(Frequency::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct RoomView {
    #[schema(example = "b0f7462c-6ca0-4a2a-9b77-1a64f1d76b2c")]
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub zones_total: Option<i64>,
    pub zones_cleaned_count: Option<i64>,
    pub last_cleaned_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NewRoom { pub name: String, pub icon: Option<String> }

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateRoom { pub name: Option<String>, pub icon: Option<String> }

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow, Clone)]
pub struct Zone {
    pub id: String,
    pub room_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub frequency: String,
    pub custom_interval_days: Option<i64>,
    pub last_cleaned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct ZoneView {
    pub id: String,
    pub room_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub frequency: String,
    pub custom_interval_days: Option<i64>,
    pub last_cleaned_at: Option<DateTime<Utc>>,
    pub next_due_at: Option<DateTime<Utc>>,
    pub is_due: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NewZone {
    pub name: String,
    pub icon: Option<String>,
    pub frequency: Frequency,
    pub custom_interval_days: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateZone {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub frequency: Option<Frequency>,
    pub custom_interval_days: Option<u16>,
}

pub fn compute_next_due(last: Option<DateTime<Utc>>, freq: &str, custom: Option<i64>) -> Option<DateTime<Utc>> {
    let last = last?;
    match Frequency::from_str(freq) {
        Some(Frequency::Daily) => Some(last + chrono::Duration::days(1)),
        Some(Frequency::Weekly) => Some(last + chrono::Duration::weeks(1)),
        Some(Frequency::Monthly) => Some(last + chrono::Duration::days(30)), // упрощённо
        Some(Frequency::Custom) => Some(last + chrono::Duration::days(custom.unwrap_or(1))),
        None => None,
    }
}

pub fn compute_is_due(next_due: Option<DateTime<Utc>>) -> bool {
    match next_due {
        Some(dt) => chrono::Utc::now() >= dt,
        None => true, // если уборки не было — просрочено
    }
}
