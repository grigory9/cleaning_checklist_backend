use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use cleaner_api::{
    api::{admin, rooms, users},
    models::{AppState, AuthResponse, RoomView},
};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env, sync::Arc};
use tower::ServiceExt;

fn test_app(state: Arc<AppState>) -> Router {
    let admin_routes = Router::new()
        .route("/clients", post(admin::create_client).get(admin::list_clients));

    let api_routes = Router::new()
        .route("/register", post(users::register))
        .route("/login", post(users::login))
        .route("/rooms", get(rooms::list_rooms).post(rooms::create_room))
        .route(
            "/rooms/:id",
            get(rooms::get_room),
        );

    Router::new()
        .nest("/api/v1", api_routes)
        .nest("/admin", admin_routes)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            |axum::extract::State(state): axum::extract::State<Arc<AppState>>, req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| async move {
                let (mut parts, body) = req.into_parts();
                parts.extensions.insert(state);
                let req = axum::http::Request::from_parts(parts, body);
                next.run(req).await
            }
        ))
        .with_state(state)
}

async fn setup_test_env() -> Arc<AppState> {
    env::set_var("JWT_SECRET", "test-jwt-secret-key-for-isolation-tests");

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    Arc::new(AppState { pool })
}

async fn create_client_and_register_user(app: Router, username: &str, email: &str, password: &str) -> AuthResponse {
    // Create OAuth client first
    let create_client_payload = json!({
        "name": "Test Client",
        "redirect_uris": ["http://localhost"],
        "grant_types": ["authorization_code", "refresh_token"],
        "scopes": ["rooms:read", "rooms:write", "zones:read", "zones:write", "stats:read"],
        "is_public": true
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/admin/clients")
                .header("content-type", "application/json")
                .body(Body::from(create_client_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let client: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let client_id = client["client_id"].as_str().unwrap();

    // Register user
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password,
        "name": format!("{} Name", username),
        "client_id": client_id
    });

    let response = app
        .oneshot(
            Request::post("/api/v1/register")
                .header("content-type", "application/json")
                .body(Body::from(register_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_room(app: Router, access_token: &str, name: &str) -> RoomView {
    let room_payload = json!({ "name": name });

    let response = app
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", access_token))
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn list_rooms(app: Router, access_token: &str) -> Vec<RoomView> {
    let response = app
        .oneshot(
            Request::get("/api/v1/rooms")
                .header("authorization", format!("Bearer {}", access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn get_room_by_id(app: Router, access_token: &str, room_id: &str) -> StatusCode {
    let response = app
        .oneshot(
            Request::get(&format!("/api/v1/rooms/{}", room_id))
                .header("authorization", format!("Bearer {}", access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    response.status()
}

#[tokio::test]
async fn test_user_data_isolation() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Create two users
    let user1_auth = create_client_and_register_user(
        app.clone(),
        "user1",
        "user1@example.com",
        "password123"
    ).await;

    let user2_auth = create_client_and_register_user(
        app.clone(),
        "user2",
        "user2@example.com",
        "password123"
    ).await;

    // User 1 creates a room
    let user1_room = create_room(app.clone(), &user1_auth.access_token, "User 1 Room").await;

    // User 2 creates a room
    let user2_room = create_room(app.clone(), &user2_auth.access_token, "User 2 Room").await;

    println!("User 1 room ID: {}", user1_room.id);
    println!("User 2 room ID: {}", user2_room.id);

    // Test 1: Users should only see their own rooms
    let user1_rooms = list_rooms(app.clone(), &user1_auth.access_token).await;
    let user2_rooms = list_rooms(app.clone(), &user2_auth.access_token).await;

    assert_eq!(user1_rooms.len(), 1);
    assert_eq!(user1_rooms[0].name, "User 1 Room");
    assert_eq!(user1_rooms[0].id, user1_room.id);

    assert_eq!(user2_rooms.len(), 1);
    assert_eq!(user2_rooms[0].name, "User 2 Room");
    assert_eq!(user2_rooms[0].id, user2_room.id);

    println!("✅ Users can only see their own rooms");

    // Test 2: User 1 should not be able to access User 2's room by ID
    let status = get_room_by_id(app.clone(), &user1_auth.access_token, &user2_room.id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    println!("✅ User 1 cannot access User 2's room by ID");

    // Test 3: User 2 should not be able to access User 1's room by ID
    let status = get_room_by_id(app.clone(), &user2_auth.access_token, &user1_room.id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    println!("✅ User 2 cannot access User 1's room by ID");

    // Test 4: Users can access their own rooms by ID
    let status = get_room_by_id(app.clone(), &user1_auth.access_token, &user1_room.id).await;
    assert_eq!(status, StatusCode::OK);

    let status = get_room_by_id(app.clone(), &user2_auth.access_token, &user2_room.id).await;
    assert_eq!(status, StatusCode::OK);

    println!("✅ Users can access their own rooms by ID");

    println!("\n🎉 User data isolation test completed successfully!");
    println!("✅ Each user can only see and access their own data");
    println!("✅ Cross-user data access is properly prevented");
}