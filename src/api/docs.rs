use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::{
    oauth::{self, RegisterRequest},
    rooms,
    stats::{self, StatsOverview},
    zones::{self, BulkClean, BulkCleanResponse, CleanBody},
};

use crate::models::{
    AuthResponse, Frequency, LoginRequest, NewRoom, NewZone, OAuthRevokeRequest, OAuthTokenRequest, OAuthTokenResponse,
    Room, RoomView, UpdateRoom, UpdateZone, UserInfoResponse, Zone, ZoneView,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        oauth::register,
        oauth::login,
        oauth::oauth_token,
        oauth::oauth_revoke,
        oauth::userinfo,
        rooms::list_rooms,
        rooms::create_room,
        rooms::get_room,
        rooms::update_room,
        rooms::delete_room,
        rooms::restore_room,
        zones::list_zones,
        zones::create_zone,
        zones::get_zone,
        zones::update_zone,
        zones::delete_zone,
        zones::clean_zone,
        zones::bulk_clean,
        stats::overview,
        stats::zones_due,
    ),
    components(schemas(
        AuthResponse,
        LoginRequest,
        RegisterRequest,
        OAuthTokenRequest,
        OAuthTokenResponse,
        OAuthRevokeRequest,
        UserInfoResponse,
        Room,
        RoomView,
        NewRoom,
        UpdateRoom,
        Zone,
        ZoneView,
        NewZone,
        UpdateZone,
        Frequency,
        CleanBody,
        BulkClean,
        BulkCleanResponse,
        StatsOverview,
    )),
    tags(
        (name = "oauth", description = "Authentication and OAuth 2.0 endpoints"),
        (name = "rooms", description = "Operations with rooms"),
        (name = "zones", description = "Operations with zones"),
        (name = "stats", description = "Statistics overview"),
    ),
    servers((url = "/api/v1"))
)]
pub struct ApiDoc;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi())
}

use std::sync::Arc;

use axum::Router;
use crate::models::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().merge(swagger_ui())
}
