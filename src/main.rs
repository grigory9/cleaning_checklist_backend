use std::{env, net::SocketAddr, sync::Arc};
use rand::Rng;

use axum::{
    extract::Request,
    routing::{get, post},
    middleware::{self, Next},
    Router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sqlx::sqlite::SqlitePoolOptions;

mod api;
mod error;
mod models;

use api::{docs, api_router};
use error::{AppError, AppResult};


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
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Other(e.into()))?;

    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
        // Генерируем случайный секрет по умолчанию для разработки
        let mut rng = rand::thread_rng();
        let secret: [u8; 32] = rng.gen();
        hex::encode(secret)
    });

    let state = Arc::new(models::AppState { pool, jwt_secret });

    let app = Router::new()
        .nest("/api/v1", api_router())
        .merge(docs::swagger_ui())
        .with_state(state)
        .layer(middleware::from_fn(|mut req: Request, next: Next| async move {
            // Добавляем JWT секрет в extensions для использования в simple_auth_middleware
            let jwt_secret = req.extensions()
                .get::<Arc<models::AppState>>()
                .map(|state| state.jwt_secret.clone());
            
            if let Some(secret) = jwt_secret {
                req.extensions_mut().insert(secret);
            }
            next.run(req).await
        }));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!(%addr, "🚀 cleaner-api запущен");

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
