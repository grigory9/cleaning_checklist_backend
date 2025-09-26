use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use cleaner_api::{
    api::{admin, rooms, stats, users, zones},
    auth::oauth::{authorize, introspect, revoke, token},
    models::{AppState, AuthResponse, RoomView},
};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env, sync::Arc};
use tower::ServiceExt;

fn test_app(state: Arc<AppState>) -> Router {
    let oauth_routes = Router::new()
        .route("/authorize", get(authorize::authorize_get).post(authorize::authorize_post))
        .route("/token", post(token::token))
        .route("/introspect", post(introspect::introspect))
        .route("/revoke", post(revoke::revoke));

    let admin_routes = Router::new()
        .route("/clients", post(admin::create_client).get(admin::list_clients));

    let api_routes = Router::new()
        .route("/register", post(users::register))
        .route("/login", post(users::login))
        .route("/me", get(users::me))
        .route("/rooms", get(rooms::list_rooms).post(rooms::create_room))
        .route(
            "/rooms/:id",
            get(rooms::get_room)
                .patch(rooms::update_room)
                .delete(rooms::delete_room),
        )
        .route("/rooms/:id/restore", post(rooms::restore_room))
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
        .route("/stats/overview", get(stats::overview))
        .route("/zones/due", get(stats::zones_due));

    Router::new()
        .nest("/api/v1", api_routes)
        .nest("/oauth", oauth_routes)
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
    // Set environment variables for JWT and database
    env::set_var("JWT_SECRET", "test-jwt-secret-key-for-integration-tests");

    // Use in-memory database for testing
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    // Run migrations to ensure the database is set up
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    Arc::new(AppState { pool })
}

async fn create_internal_oauth_client(app: Router) -> String {
    let create_client_payload = json!({
        "name": "Internal Client",
        "redirect_uris": ["http://localhost"],
        "grant_types": ["authorization_code", "refresh_token"],
        "scopes": ["rooms:read", "rooms:write", "zones:read", "zones:write", "stats:read"],
        "is_public": true
    });

    let response = app
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

    client["client_id"].as_str().unwrap().to_string()
}

async fn register_user_with_jwt(app: Router, username: &str, email: &str, password: &str, name: &str, client_id: &str) -> AuthResponse {
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password,
        "name": name,
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

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

    if status != StatusCode::OK {
        let error_text = String::from_utf8_lossy(&body);
        panic!("Registration failed with status {}: {}", status, error_text);
    }

    serde_json::from_slice(&body).unwrap()
}

async fn create_room_with_auth(app: Router, access_token: &str, name: &str, icon: Option<&str>) -> RoomView {
    let mut room_payload = json!({ "name": name });
    if let Some(icon_val) = icon {
        room_payload["icon"] = json!(icon_val);
    }

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

async fn get_room_by_id(app: Router, access_token: &str, room_id: &str) -> RoomView {
    let response = app
        .oneshot(
            Request::get(&format!("/api/v1/rooms/{}", room_id))
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

async fn list_rooms_with_auth(app: Router, access_token: &str) -> Vec<RoomView> {
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

#[tokio::test]
async fn test_register_create_get_room_flow() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // First, create an internal OAuth client for JWT tokens
    let client_id = create_internal_oauth_client(app.clone()).await;

    // Step 1: Register a new user and get JWT token
    let auth_response = register_user_with_jwt(
        app.clone(),
        "testuser",
        "test@example.com",
        "securepassword123",
        "Test User",
        &client_id
    ).await;

    // Verify the registration response
    assert_eq!(auth_response.token_type, "Bearer");
    assert_eq!(auth_response.expires_in, 86400); // 24 hours
    assert!(!auth_response.access_token.is_empty());
    assert_eq!(auth_response.user.username, "testuser");
    assert_eq!(auth_response.user.email, "test@example.com");
    assert_eq!(auth_response.user.name, Some("Test User".to_string()));
    assert_eq!(auth_response.user.email_verified, false);

    println!("✅ User registered successfully with JWT token");

    // Step 2: Create a room using the JWT token
    let created_room = create_room_with_auth(
        app.clone(),
        &auth_response.access_token,
        "Living Room",
        Some("🛋️")
    ).await;

    // Verify room creation
    assert_eq!(created_room.name, "Living Room");
    assert_eq!(created_room.icon, Some("🛋️".to_string()));
    assert_eq!(created_room.zones_total, Some(0));
    assert_eq!(created_room.zones_cleaned_count, Some(0));
    assert!(!created_room.id.is_empty());

    println!("✅ Room created successfully: {} ({})", created_room.name, created_room.id);

    // Step 3: Get the specific room by ID
    let fetched_room = get_room_by_id(
        app.clone(),
        &auth_response.access_token,
        &created_room.id
    ).await;

    // Verify fetched room matches created room
    assert_eq!(fetched_room.id, created_room.id);
    assert_eq!(fetched_room.name, "Living Room");
    assert_eq!(fetched_room.icon, Some("🛋️".to_string()));
    assert_eq!(fetched_room.zones_total, Some(0));
    assert_eq!(fetched_room.zones_cleaned_count, Some(0));

    println!("✅ Room fetched successfully by ID: {}", fetched_room.id);

    // Step 4: List all rooms and verify our room is included
    let rooms = list_rooms_with_auth(app.clone(), &auth_response.access_token).await;

    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].id, created_room.id);
    assert_eq!(rooms[0].name, "Living Room");
    assert_eq!(rooms[0].icon, Some("🛋️".to_string()));

    println!("✅ Room listing successful, found {} room(s)", rooms.len());

    // Step 5: Create a second room to test multiple rooms
    let second_room = create_room_with_auth(
        app.clone(),
        &auth_response.access_token,
        "Kitchen",
        Some("🍳")
    ).await;

    assert_eq!(second_room.name, "Kitchen");
    assert_eq!(second_room.icon, Some("🍳".to_string()));

    println!("✅ Second room created: {} ({})", second_room.name, second_room.id);

    // Step 6: Verify both rooms are listed
    let all_rooms = list_rooms_with_auth(app.clone(), &auth_response.access_token).await;
    assert_eq!(all_rooms.len(), 2);

    let room_names: Vec<String> = all_rooms.iter().map(|r| r.name.clone()).collect();
    assert!(room_names.contains(&"Living Room".to_string()));
    assert!(room_names.contains(&"Kitchen".to_string()));

    println!("✅ Both rooms listed successfully: {:?}", room_names);

    println!("\n🎉 Integration test completed successfully!");
    println!("✅ User registration with JWT token");
    println!("✅ Room creation with authentication");
    println!("✅ Room retrieval by ID");
    println!("✅ Room listing with multiple rooms");
    println!("✅ All operations work correctly with real database");
}

#[tokio::test]
async fn test_authentication_required() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Try to create a room without authentication
    let room_payload = json!({ "name": "Unauthorized Room" });
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Try to list rooms without authentication
    let response = app
        .oneshot(
            Request::get("/api/v1/rooms")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    println!("✅ Authentication is properly required for protected endpoints");
}

#[tokio::test]
async fn test_invalid_jwt_token() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Try to create a room with invalid JWT token
    let room_payload = json!({ "name": "Invalid Token Room" });
    let response = app
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .header("authorization", "Bearer invalid-jwt-token")
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    println!("✅ Invalid JWT tokens are properly rejected");
}