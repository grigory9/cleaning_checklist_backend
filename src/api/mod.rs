pub mod rooms;
pub mod zones;
pub mod stats;
pub mod docs;

#[cfg(test)]
mod tests {
    use super::rooms;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::models::{AppState, RoomView};

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/rooms", get(rooms::list_rooms).post(rooms::create_room))
            .with_state(state)
    }

    #[tokio::test]
    async fn create_and_list_room() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = Arc::new(AppState { pool });

        let app = test_app(state);

        let payload = json!({ "name": "kitchen", "icon": null });
        let response = app
            .clone()
            .oneshot(
                Request::post("/rooms")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let created: RoomView = serde_json::from_slice(&body).unwrap();
        assert_eq!(created.name, "kitchen");

        let response = app
            .oneshot(Request::get("/rooms").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let rooms: Vec<RoomView> = serde_json::from_slice(&body).unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].name, "kitchen");
    }
}
