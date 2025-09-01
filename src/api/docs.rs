use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::{
    rooms,
    stats::{self, StatsOverview},
    zones::{self, BulkClean, BulkCleanResponse, CleanBody},
};

use crate::models::{
    Frequency, NewRoom, NewZone, Room, RoomView, UpdateRoom, UpdateZone, Zone, ZoneView,
};

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
        stats::zones_due,
    ),
    components(schemas(
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
