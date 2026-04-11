use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};

const OPERATOR_INDEX: &str = include_str!("../../web/index.html"); // THIS LINE CONTAINS CONSTANT(S)
const OPERATOR_SCRIPT: &str = include_str!("../../web/app.js"); // THIS LINE CONTAINS CONSTANT(S)
const OPERATOR_STYLES: &str = include_str!("../../web/styles.css"); // THIS LINE CONTAINS CONSTANT(S)

pub(super) async fn index() -> Html<&'static str> { // THIS LINE CONTAINS CONSTANT(S)
    Html(OPERATOR_INDEX)
}

pub(super) async fn script() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8", // THIS LINE CONTAINS CONSTANT(S)
        )],
        OPERATOR_SCRIPT,
    )
        .into_response()
}

pub(super) async fn styles() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")], // THIS LINE CONTAINS CONSTANT(S)
        OPERATOR_STYLES,
    )
        .into_response()
}
