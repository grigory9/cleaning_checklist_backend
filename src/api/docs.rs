use axum::Router;
use utoipa::{OpenApi, Modify, openapi::security::{ApiKey, ApiKeyValue, SecurityScheme}};
use utoipa_swagger_ui::SwaggerUi;

use super::{rooms, zones, stats};
use crate::models::{RoomView, NewRoom, UpdateRoom, ZoneView, NewZone, UpdateZone};

#[derive(OpenApi)]
#[openapi(
    paths(
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
        stats::zones_due
    ),
    components(schemas(RoomView, NewRoom, UpdateRoom, ZoneView, NewZone, UpdateZone)),
    tags(
        (name = "rooms", description = "Операции с комнатами"),
        (name = "zones", description = "Операции с зонами"),
        (name = "stats", description = "Сводки и due")
    )
)]
pub struct ApiDoc;

pub fn swagger() -> Router {
    SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()).into()
}
