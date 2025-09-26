use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post, patch, delete},
    Router,
};
use cleaner_api::{
    api::{admin, rooms, stats, users, zones},
    auth::oauth::{authorize, introspect, revoke, token},
    models::{AppState, RoomView, UserView},
};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
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
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    Arc::new(AppState { pool })
}

async fn create_oauth_client(app: Router) -> (String, String) {
    let create_client_payload = json!({
        "name": "Test Client",
        "redirect_uris": ["http://localhost:3000/callback"],
        "grant_types": ["authorization_code", "refresh_token"],
        "scopes": ["rooms:read", "rooms:write", "zones:read", "zones:write", "stats:read"],
        "is_public": false
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

    (
        client["client_id"].as_str().unwrap().to_string(),
        client["client_secret"].as_str().unwrap().to_string(),
    )
}

async fn register_user(app: Router, username: &str, email: &str, password: &str) -> String {
    let register_payload = json!({
        "username": username,
        "email": email,
        "password": password
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
    let user: UserView = serde_json::from_slice(&body).unwrap();
    user.id
}

async fn get_authorization_code(app: Router, client_id: &str, user_id: &str) -> String {
    let form_data = format!(
        "user_id={}&client_id={}&redirect_uri={}&scope={}&approved=true",
        user_id,
        client_id,
        "http://localhost:3000/callback",
        "rooms:read%20rooms:write%20zones:read%20zones:write%20stats:read"
    );

    let response = app
        .oneshot(
            Request::post("/oauth/authorize")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form_data))
                .unwrap(),
        )
        .await
        .unwrap();

    // Accept both 302 (Found) and 303 (See Other) as valid redirect responses
    assert!(
        response.status() == StatusCode::FOUND || response.status() == StatusCode::SEE_OTHER,
        "Expected redirect status, got: {}", response.status()
    );
    let location = response.headers().get("location").unwrap().to_str().unwrap();

    // Extract code from redirect URL
    let url = url::Url::parse(location).unwrap();
    url.query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.to_string())
        .unwrap()
}

async fn exchange_code_for_token(app: Router, client_id: &str, client_secret: &str, code: &str) -> String {
    let form_data = format!(
        "grant_type=authorization_code&client_id={}&client_secret={}&code={}&redirect_uri={}",
        client_id,
        client_secret,
        code,
        "http://localhost:3000/callback"
    );

    let response = app
        .oneshot(
            Request::post("/oauth/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(form_data))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let token_response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    token_response["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn register_create_room_get_rooms_integration_test() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Create OAuth client
    let (client_id, client_secret) = create_oauth_client(app.clone()).await;

    // Register user
    let user_id = register_user(app.clone(), "testuser", "test@example.com", "password123").await;

    // Get authorization code
    let code = get_authorization_code(app.clone(), &client_id, &user_id).await;

    // Exchange code for access token
    let access_token = exchange_code_for_token(app.clone(), &client_id, &client_secret, &code).await;

    // Test creating a room with authentication
    let room_payload = json!({ "name": "Living Room", "icon": "🛋️" });
    let response = app
        .clone()
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
    let created_room: RoomView = serde_json::from_slice(&body).unwrap();
    assert_eq!(created_room.name, "Living Room");
    assert_eq!(created_room.icon, Some("🛋️".to_string()));
    assert_eq!(created_room.zones_total, Some(0));
    assert_eq!(created_room.zones_cleaned_count, Some(0));

    // Test listing rooms with authentication
    let response = app
        .clone()
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
    let rooms: Vec<RoomView> = serde_json::from_slice(&body).unwrap();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].name, "Living Room");

    // Test creating another room
    let room_payload2 = json!({ "name": "Kitchen", "icon": "🍳" });
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", access_token))
                .body(Body::from(room_payload2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test listing rooms again - should have 2
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
    let rooms: Vec<RoomView> = serde_json::from_slice(&body).unwrap();
    assert_eq!(rooms.len(), 2);
}

#[tokio::test]
async fn test_user_isolation() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Create OAuth client
    let (client_id, client_secret) = create_oauth_client(app.clone()).await;

    // Register first user
    let user1_id = register_user(app.clone(), "user1", "user1@example.com", "password123").await;
    let code1 = get_authorization_code(app.clone(), &client_id, &user1_id).await;
    let token1 = exchange_code_for_token(app.clone(), &client_id, &client_secret, &code1).await;

    // Register second user
    let user2_id = register_user(app.clone(), "user2", "user2@example.com", "password123").await;
    let code2 = get_authorization_code(app.clone(), &client_id, &user2_id).await;
    let token2 = exchange_code_for_token(app.clone(), &client_id, &client_secret, &code2).await;

    // User 1 creates a room
    let room_payload = json!({ "name": "User1's Room", "icon": "🏠" });
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token1))
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // User 2 creates a room
    let room_payload = json!({ "name": "User2's Room", "icon": "🏡" });
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rooms")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::from(room_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // User 1 should only see their own room
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/rooms")
                .header("authorization", format!("Bearer {}", token1))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let rooms: Vec<RoomView> = serde_json::from_slice(&body).unwrap();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].name, "User1's Room");

    // User 2 should only see their own room
    let response = app
        .oneshot(
            Request::get("/api/v1/rooms")
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let rooms: Vec<RoomView> = serde_json::from_slice(&body).unwrap();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].name, "User2's Room");
}

#[tokio::test]
async fn test_unauthorized_access() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Try to access protected endpoint without token
    let response = app
        .oneshot(
            Request::get("/api/v1/rooms")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_token_introspection() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Create OAuth client and get token
    let (client_id, client_secret) = create_oauth_client(app.clone()).await;
    let user_id = register_user(app.clone(), "testuser", "test@example.com", "password123").await;
    let code = get_authorization_code(app.clone(), &client_id, &user_id).await;
    let access_token = exchange_code_for_token(app.clone(), &client_id, &client_secret, &code).await;

    // Test token introspection
    let introspect_payload = json!({ "token": access_token });
    let response = app
        .oneshot(
            Request::post("/oauth/introspect")
                .header("content-type", "application/json")
                .body(Body::from(introspect_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let introspection: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(introspection["active"], true);
    assert_eq!(introspection["client_id"], client_id);
    assert_eq!(introspection["username"], "testuser");
}

#[tokio::test]
async fn test_stats_endpoint() {
    let state = setup_test_env().await;
    let app = test_app(state.clone());

    // Setup authenticated user
    let (client_id, client_secret) = create_oauth_client(app.clone()).await;
    let user_id = register_user(app.clone(), "testuser", "test@example.com", "password123").await;
    let code = get_authorization_code(app.clone(), &client_id, &user_id).await;
    let access_token = exchange_code_for_token(app.clone(), &client_id, &client_secret, &code).await;

    // Test stats overview endpoint
    let response = app
        .oneshot(
            Request::get("/api/v1/stats/overview")
                .header("authorization", format!("Bearer {}", access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let stats: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(stats["rooms_total"], 0);
    assert_eq!(stats["zones_total"], 0);
    assert_eq!(stats["due_zones"], 0);
}