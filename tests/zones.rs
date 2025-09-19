use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use cleaner_api::{api::{rooms, zones}, models::{AppState, Frequency}};
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
        .route(
            "/rooms/:room_id/zones",
            get(zones::list_zones).post(zones::create_zone),
        )
        .route(
            "/zones/:id",
            get(zones::get_zone).patch(zones::update_zone),
        )
        .route("/zones/:id/clean", post(zones::clean_zone))
        .route("/rooms/:id", get(rooms::get_room));
    Router::new().nest("/api/v1", api_routes).with_state(state)
}

#[tokio::test]
async fn create_and_update_zone() {
    let app = test_app().await;

    // create room
    let room_body = json!({"name": "Kitchen"});
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

    // create zone
    let zone_body = json!({
        "name": "Table",
        "frequency": Frequency::Daily,
    });
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/rooms/{}/zones", room.id))
                .header("content-type", "application/json")
                .body(Body::from(zone_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let zone: cleaner_api::models::ZoneView = serde_json::from_slice(&body).unwrap();
    assert_eq!(zone.name, "Table");

    // update zone
    let upd_body = json!({"name": "Desk"});
    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/v1/zones/{}", zone.id))
                .header("content-type", "application/json")
                .body(Body::from(upd_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let zone: cleaner_api::models::ZoneView = serde_json::from_slice(&body).unwrap();
    assert_eq!(zone.name, "Desk");
}

#[tokio::test]
async fn room_get_includes_zone_stats() {
    let app = test_app().await;

    // create room
    let room_body = json!({"name": "Living"});
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
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let room: cleaner_api::models::RoomView = serde_json::from_slice(&body).unwrap();

    // create two zones
    for name in ["A", "B"] {
        let zone_body = json!({"name": name, "frequency": Frequency::Daily});
        app.clone()
            .oneshot(
                Request::post(format!("/api/v1/rooms/{}/zones", room.id))
                    .header("content-type", "application/json")
                    .body(Body::from(zone_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // clean one zone
    let zones_res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/rooms/{}/zones", room.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(zones_res.into_body(), usize::MAX).await.unwrap();
    let zones: Vec<cleaner_api::models::ZoneView> = serde_json::from_slice(&body).unwrap();
    let first_zone = zones.first().unwrap();
    app.clone()
        .oneshot(
            Request::post(format!("/api/v1/zones/{}/clean", first_zone.id))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    // get room and check stats
    let res = app
        .clone()
        .oneshot(Request::get(format!("/api/v1/rooms/{}", room.id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let room: cleaner_api::models::RoomView = serde_json::from_slice(&body).unwrap();
    assert_eq!(room.zones_total, Some(2));
    assert_eq!(room.zones_cleaned_count, Some(1));
}
