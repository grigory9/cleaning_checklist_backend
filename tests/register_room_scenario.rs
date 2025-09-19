use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use cleaner_api::{
    api::{oauth, rooms},
    models::{AppState, AuthResponse}
};
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
    
    // Создаем отдельные маршруты: OAuth без аутентификации, комнаты с аутентификацией
    let oauth_routes = Router::new()
        .route("/register", post(oauth::register))
        .route("/login", post(oauth::login));
    
    let protected_routes = Router::new()
        .route("/rooms", get(rooms::list_rooms).post(rooms::create_room))
        .route("/rooms/:id", get(rooms::get_room))
        .layer(axum::middleware::from_fn(oauth::simple_auth_middleware));
    
    let api_routes = Router::new()
        .nest("/oauth", oauth_routes)
        .nest("", protected_routes);
    
    Router::new()
        .nest("/api/v1", api_routes)
        .with_state(state)
        .layer(axum::middleware::from_fn(|mut req: axum::extract::Request, next: axum::middleware::Next| async move {
            // Добавляем JWT секрет в extensions для использования в simple_auth_middleware
            req.extensions_mut().insert("test-secret".to_string());
            next.run(req).await
        }))
}

#[tokio::test]
async fn register_create_room_get_room_scenario() {
    let app = test_app().await;

    // 1. Регистрация пользователя
    let register_body = json!({
        "email": "test@example.com",
        "password": "password123"
    });
    
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/oauth/register")
                .header("content-type", "application/json")
                .body(Body::from(register_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let auth_response: AuthResponse = serde_json::from_slice(&body).unwrap();
    let token = auth_response.token;

    // 2. Создание комнаты с полученным токеном
    let room_body = json!({"name": "Test Room"});
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(room_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    let status = res.status();
    if status != StatusCode::CREATED {
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let error_body = String::from_utf8_lossy(&body);
        panic!("Failed to create room. Status: {}, Body: {}", status, error_body);
    }
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let room: cleaner_api::models::RoomView = serde_json::from_slice(&body).unwrap();
    assert_eq!(room.name, "Test Room");

    // 3. Получение созданной комнаты
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/rooms/{}", room.id))
                .header("Authorization", format!("Bearer {}", token))
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