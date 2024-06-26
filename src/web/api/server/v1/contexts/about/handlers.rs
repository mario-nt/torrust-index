//! API handlers for the the [`about`](crate::web::api::server::v1::contexts::about) API
//! context.
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::common::AppData;

#[allow(clippy::unused_async)]
pub async fn about_page_handler(State(app_data): State<Arc<AppData>>) -> Response {
    /* (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        about::page(),
    )
        .into_response() */
    let html = app_data.about_service.get_about_page();

    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

#[allow(clippy::unused_async)]
pub async fn license_page_handler(State(app_data): State<Arc<AppData>>) -> Response {
    let html = app_data.about_service.get_license_page();

    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}
