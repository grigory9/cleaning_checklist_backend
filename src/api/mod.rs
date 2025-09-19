pub mod oauth;
pub mod rooms;
pub mod zones;
pub mod stats;
pub mod docs;

use std::sync::Arc;

use axum::{Router, middleware};
use crate::models::AppState;
use tower::ServiceBuilder;

pub fn api_router() -> Router<Arc<AppState>> {
    Router::new()
        .nest("/oauth", oauth::router())
        .nest("/rooms", rooms::router().layer(ServiceBuilder::new().layer(middleware::from_fn(oauth::simple_auth_middleware))))
        .nest("/zones", zones::router().layer(ServiceBuilder::new().layer(middleware::from_fn(oauth::simple_auth_middleware))))
        .nest("/stats", stats::router().layer(ServiceBuilder::new().layer(middleware::from_fn(oauth::simple_auth_middleware))))
        .nest("/docs", docs::router())
}

