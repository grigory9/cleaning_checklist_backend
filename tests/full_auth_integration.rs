use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use cleaner_api::models::AppState;
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app() -> Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let state = Arc::new(AppState {
        pool,
        jwt_secret: "test-secret-for-integration-tests".to_string(),
    });

    // Create app with proper auth middleware that has access to state
    use axum::{middleware, routing::{get, post}};
    use tower::ServiceBuilder;

    Router::new()
        .nest("/oauth", cleaner_api::api::oauth::router())
        .nest("/rooms", cleaner_api::api::rooms::router().layer(ServiceBuilder::new().layer(middleware::from_fn_with_state(state.clone(), cleaner_api::api::oauth::auth_middleware))))
        .with_state(state)
}

#[tokio::test]
async fn register_create_room_get_rooms_integration_test() {
    let app = test_app().await;

    // Step 1: Register new user
    let register_payload = json!({
        "email": "test@example.com",
        "password": "secure_password123",
        "email_verified": true
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/oauth/register")
                .header("content-type", "application/json")
                .body(Body::from(register_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let auth_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let token = auth_response["token"].as_str().unwrap();
    assert!(!token.is_empty(), "JWT token should not be empty");

    // Step 2: Create room using Bearer token
    let room_payload = json!({
        "name": "Living Room",
        "icon": "🛋️"
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/rooms")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = std::str::from_utf8(&body).unwrap();

    if status != StatusCode::CREATED {
        println!("Room creation failed. Status: {}, Response: {}", status, body_str);
        panic!("Expected 201 CREATED, got {}", status);
    }

    let room: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(room["name"], "Living Room");
    assert_eq!(room["icon"], "🛋️");
    let room_id = room["id"].as_str().unwrap();

    // Step 3: Get rooms list using Bearer token
    let response = app
        .clone()
        .oneshot(
            Request::get("/rooms")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let rooms: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let rooms_array = rooms.as_array().unwrap();
    assert_eq!(rooms_array.len(), 1);
    assert_eq!(rooms_array[0]["name"], "Living Room");
    assert_eq!(rooms_array[0]["id"], room_id);

    // Step 4: Get specific room using Bearer token
    let response = app
        .clone()
        .oneshot(
            Request::get(&format!("/rooms/{}", room_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let room: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(room["name"], "Living Room");
    assert_eq!(room["id"], room_id);
    assert_eq!(room["zones_total"], 0);
}

#[tokio::test]
async fn unauthorized_room_access_should_fail() {
    let app = test_app().await;

    // Try to access rooms without token
    let response = app
        .clone()
        .oneshot(
            Request::get("/rooms")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Try to create room without token
    let room_payload = json!({
        "name": "Unauthorized Room"
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/rooms")
                .header("content-type", "application/json")
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_token_should_fail() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::get("/rooms")
                .header("authorization", "Bearer invalid_token_here")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_flow_should_work() {
    let app = test_app().await;

    // Step 1: Register user first
    let register_payload = json!({
        "email": "logintest@example.com",
        "password": "password123"
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/oauth/register")
                .header("content-type", "application/json")
                .body(Body::from(register_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Step 2: Login with same credentials
    let login_payload = json!({
        "email": "logintest@example.com",
        "password": "password123"
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/oauth/login")
                .header("content-type", "application/json")
                .body(Body::from(login_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let auth_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let token = auth_response["token"].as_str().unwrap();
    assert!(!token.is_empty());

    // Step 3: Use login token to create room
    let room_payload = json!({
        "name": "Kitchen",
        "icon": "🍳"
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/rooms")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let room: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(room["name"], "Kitchen");
    assert_eq!(room["icon"], "🍳");
}