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
        let mut mime = guess_mime(path);
        // Next.js app router RSC payloads must be served as text/x-component
        if path.ends_with(".rsc") {
            mime = "text/x-component".parse().unwrap();
        }
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
/// Serve embedded UI assets. Maps URLs like `/`, `/frames`, `/assets/app.js` to corresponding files.
pub async fn serve_ui(uri: Uri) -> Response {
    let raw_path = uri.path().trim_start_matches('/');
    let mut tried: Vec<String> = Vec::new();
    if raw_path.is_empty() {
        if let Some(resp) = respond("index.html") {
            return resp;
        }
        tried.push("index.html".into());
    } else {
        // Try exact file, `path/index.html`, and `path.html`
        let candidates = [
            raw_path.to_string(),
            format!("{}/index.html", raw_path),
            format!("{}.html", raw_path),
        ];
        for cand in &candidates {
            if let Some(resp) = respond(cand) {
                return resp;
            }
            tried.push(cand.clone());
        }
    }
    // Fallback: try Next.js 404 if present, otherwise 404
    if let Some(resp) = respond("404.html") {
        return resp;
    }
    axum::http::Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
