use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

mod api;
mod error;
mod models;

use api::{docs, rooms, stats, zones};
use error::AppResult;

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenvy::dotenv().ok();

    let env_filter = env::var("RUST_LOG")
        .unwrap_or_else(|_| "cleaner_api=info,axum=info,tower_http=info".to_string());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port: u16 = env::var("APP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        // по умолчанию локальный файл
        "sqlite://./cleaner.db".to_string()
    });

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // Миграции (каталог migrations)
    sqlx::migrate!("./migrations").run(&pool).await?;

    let state = Arc::new(models::AppState { pool });

    let api_routes = Router::new()
        // Rooms
        .route("/rooms", get(rooms::list_rooms).post(rooms::create_room))
        .route(
            "/rooms/:id",
            get(rooms::get_room)
                .patch(rooms::update_room)
                .delete(rooms::delete_room),
        )
        .route("/rooms/:id/restore", post(rooms::restore_room))
        // Zones
        .route(
            "/rooms/:room_id/zones",
            get(zones::list_zones).post(zones::create_zone),
        )
        .route(
            "/zones/:id",
            get(zones::get_zone)
                .patch(zones::update_zone)
                .delete(zones::delete_zone),
        )
        .route("/zones/:id/clean", post(zones::clean_zone))
        .route("/zones/bulk/clean", post(zones::bulk_clean))
        // Stats
        .route("/stats/overview", get(stats::overview))
        .route("/zones/due", get(stats::zones_due));

    let app = Router::new()
        .nest("/api/v1", api_routes)
        .merge(docs::swagger())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!(%addr, "🚀 cleaner-api запущен");

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
