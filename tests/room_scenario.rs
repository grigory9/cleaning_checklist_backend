use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use cleaner_api::{api::rooms, models::AppState};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower::ServiceExt; // for oneshot

async fn test_app() -> Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let state = Arc::new(AppState {
        pool,
        jwt_secret: "test-secret".to_string(),
    });
    
    let api_routes = Router::new()
        .route("/rooms", get(rooms::list_rooms).post(rooms::create_room))
        .route("/rooms/:id", get(rooms::get_room));
    
    Router::new().nest("/api/v1", api_routes).with_state(state)
}

#[tokio::test]
async fn create_room_and_get_room_scenario() {
    let app = test_app().await;

    // 1. Создание комнаты
    let room_body = json!({"name": "Test Room"});
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .body(Body::from(room_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let room: cleaner_api::models::RoomView = serde_json::from_slice(&body).unwrap();
    assert_eq!(room.name, "Test Room");

    // 2. Получение созданной комнаты
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/rooms/{}", room.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let fetched_room: cleaner_api::models::RoomView = serde_json::from_slice(&body).unwrap();
    assert_eq!(fetched_room.id, room.id);
    assert_eq!(fetched_room.name, "Test Room");
}