//! UI embedding and static file serving.
//! When the `embed_ui` feature is enabled, we embed the Next.js exported site
//! from `../photoframe-nextjs/out` and serve it via axum routes.

#[cfg(feature = "embed_ui")]
use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::Response,
};

#[cfg(feature = "embed_ui")]
use rust_embed::RustEmbed;
use tracing::debug;

#[cfg(feature = "embed_ui")]
#[derive(RustEmbed)]
#[folder = "../photoframe-nextjs/out"]
struct UiAssets;

#[cfg(feature = "embed_ui")]
fn guess_mime(path: &str) -> mime::Mime {
    mime_guess::from_path(path).first_or_octet_stream()
}

#[cfg(feature = "embed_ui")]
fn respond(path: &str) -> Option<Response> {
    UiAssets::get(path).map(|file| {
        let mime = guess_mime(path);
        let cache = if path.ends_with(".html") {
            "no-cache"
        } else if path.contains("_next/static") {
            "public, max-age=31536000, immutable"
        } else {
            "public, max-age=86400"
        };
        let mut resp = axum::http::Response::new(Body::from(file.data.into_owned()));
        let headers = resp.headers_mut();
        headers.insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
        headers.insert(header::CACHE_CONTROL, cache.parse().unwrap());
        resp
    })
}

#[cfg(feature = "embed_ui")]
pub async fn serve_ui(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(resp) = respond(path) {
        return resp;
    }

    let html_path = format!("{}.html", path);
    if let Some(resp) = respond(&html_path) {
        return resp;
    }

    let index_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        format!("{}/index.html", path)
    };
    if let Some(resp) = respond(&index_path) {
        return resp;
    }

    debug!(path = %path, "Embedded UI asset not found; falling back to 404");
    if let Some(resp) = respond("404.html") {
        return resp;
    }

    axum::http::Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
