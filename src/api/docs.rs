use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

use super::{
    admin,
    rooms,
    stats::{self, StatsOverview},
    users,
    zones::{self, BulkClean, BulkCleanResponse, CleanBody},
};

use crate::{
    auth::oauth::{authorize, introspect, revoke, token},
    models::{
        Frequency, IntrospectRequest, IntrospectResponse, LoginUser, NewRoom, NewZone,
        OAuthClient, OAuthClientCredentials, RegisterUser, RevokeRequest, Room, RoomView,
        TokenResponse, UpdateRoom, UpdateZone, User, UserView, Zone, ZoneView,
    },
};

use crate::auth::oauth::{
    authorize::ConsentForm,
    token::TokenRequest,
};

use super::admin::CreateClientRequest;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        )
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        // User Management
        users::register,
        users::login,
        users::me,
        // OAuth2.0 Endpoints
        authorize::authorize_get,
        authorize::authorize_post,
        token::token,
        introspect::introspect,
        revoke::revoke,
        // Admin Endpoints
        admin::create_client,
        admin::list_clients,
        // Rooms API
        rooms::list_rooms,
        rooms::create_room,
        rooms::get_room,
        rooms::update_room,
        rooms::delete_room,
        rooms::restore_room,
        // Zones API
        zones::list_zones,
        zones::create_zone,
        zones::get_zone,
        zones::update_zone,
        zones::delete_zone,
        zones::clean_zone,
        zones::bulk_clean,
        // Stats API
        stats::overview,
        stats::zones_due,
    ),
    components(schemas(
        // User Models
        User,
        UserView,
        RegisterUser,
        LoginUser,
        // OAuth Models
        OAuthClient,
        OAuthClientCredentials,
        TokenResponse,
        TokenRequest,
        ConsentForm,
        IntrospectRequest,
        IntrospectResponse,
        RevokeRequest,
        CreateClientRequest,
        // Room Models
        Room,
        RoomView,
        NewRoom,
        UpdateRoom,
        // Zone Models
        Zone,
        ZoneView,
        NewZone,
        UpdateZone,
        Frequency,
        CleanBody,
        BulkClean,
        BulkCleanResponse,
        // Stats Models
        StatsOverview,
    )),
    tags(
        (name = "auth", description = "User authentication and registration"),
        (name = "oauth", description = "OAuth2.0 authorization server endpoints"),
        (name = "admin", description = "Administrative operations for OAuth clients"),
        (name = "rooms", description = "Room management operations"),
        (name = "zones", description = "Zone management operations"),
        (name = "stats", description = "Statistics and reporting"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    modifiers(&SecurityAddon),
    servers(
        (url = "/api/v1", description = "API v1 endpoints"),
        (url = "/oauth", description = "OAuth2.0 endpoints"),
        (url = "/admin", description = "Admin endpoints")
    )
)]
pub struct ApiDoc;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi())
}
