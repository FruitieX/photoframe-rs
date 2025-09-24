use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, instrument};

use crate::frame;
#[cfg(feature = "embed_ui")]
use crate::ui;
use crate::{config, scheduler};
use std::str::FromStr;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub cfg: config::SharedConfig,
    pub scheduler: std::sync::Arc<scheduler::FrameScheduler>,
}

#[derive(Deserialize)]
pub struct FrameUpdate {
    #[serde(default)]
    pub dithering: Option<String>,
    #[serde(default)]
    pub brightness: Option<f32>,
    #[serde(default)]
    pub contrast: Option<f32>,
    #[serde(default)]
    pub saturation: Option<f32>,
    #[serde(default)]
    pub sharpness: Option<f32>,
    #[serde(default)]
    pub left: Option<i32>,
    #[serde(default)]
    pub right: Option<i32>,
    #[serde(default)]
    pub top: Option<i32>,
    #[serde(default)]
    pub bottom: Option<i32>,
    #[serde(default)]
    pub paused: Option<bool>,
    #[serde(default)]
    pub dummy: Option<bool>,
    #[serde(default)]
    pub flip: Option<bool>,
}

#[derive(Serialize)]
pub struct FrameResponse {
    pub id: String,
    pub dithering: Option<String>,
    pub adjustments: Option<crate::config::Adjustments>,
    pub overscan: Option<crate::config::Overscan>,
    pub paused: bool,
    pub dummy: bool,
    pub flip: bool,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub frame_id: String,
    pub width: u32,
    pub height: u32,
}

// Logs all 4xx/5xx responses with method, URI, status and latency.
async fn log_error_responses(req: axum::extract::Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();
    let res = next.run(req).await;
    let status = res.status();
    if status.is_server_error() {
        tracing::error!(%method, %uri, %status, elapsed_ms = start.elapsed().as_millis(), "http 5xx");
    } else if status.is_client_error() {
        tracing::warn!(%method, %uri, %status, elapsed_ms = start.elapsed().as_millis(), "http 4xx");
    }
    res
}

pub async fn get_config(State(state): State<AppState>) -> Result<Json<config::Config>, StatusCode> {
    config::ConfigManager::to_struct(&state.cfg)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn patch_frame(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<FrameUpdate>,
) -> Result<Json<FrameResponse>, StatusCode> {
    // Only persist configuration; do not push to device here.
    if let Some(d) = &payload.dithering {
        config::ConfigManager::set_frame_dithering(&state.cfg, &frame_id, d)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    if payload.brightness.is_some()
        || payload.contrast.is_some()
        || payload.saturation.is_some()
        || payload.sharpness.is_some()
    {
        config::ConfigManager::update_frame_adjustments(
            &state.cfg,
            &frame_id,
            payload.brightness,
            payload.contrast,
            payload.saturation,
            payload.sharpness,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    if payload.left.is_some()
        || payload.right.is_some()
        || payload.top.is_some()
        || payload.bottom.is_some()
    {
        config::ConfigManager::update_frame_overscan(
            &state.cfg,
            &frame_id,
            payload.left,
            payload.right,
            payload.top,
            payload.bottom,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    if let Some(pause) = payload.paused {
        config::ConfigManager::set_frame_paused(&state.cfg, &frame_id, pause)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    if let Some(d) = payload.dummy {
        config::ConfigManager::set_frame_dummy(&state.cfg, &frame_id, d)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    if let Some(flip) = payload.flip {
        config::ConfigManager::set_frame_flip(&state.cfg, &frame_id, flip)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    config::ConfigManager::save(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cfg = config::ConfigManager::to_struct(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(frame) = cfg.photoframes.get(&frame_id) {
        let resp = FrameResponse {
            id: frame_id,
            dithering: frame.dithering.clone(),
            adjustments: frame.adjustments.clone(),
            overscan: frame.overscan.clone(),
            paused: frame.paused,
            dummy: frame.dummy,
            flip: frame.flip.unwrap_or(false),
        };
        Ok(Json(resp))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        .on_failure(DefaultOnFailure::new().level(Level::ERROR));

    // Build API router and mount it under /api
    let api = Router::new()
        .route("/config", get(get_config))
        .route("/frames/{id}", patch(patch_frame))
        .route("/frames/{id}/clear", post(clear_frame))
        .route("/frames/{id}/palette", get(frame_palette))
        .route("/frames/{id}/intermediate", get(get_intermediate_image))
        .route(
            "/frames/{id}/upload",
            post(upload_frame).layer(DefaultBodyLimit::disable()),
        )
        .route("/frames/{id}/trigger", post(trigger_frame))
        .route("/frames/{id}/next", post(next_frame))
        .route("/frames/{id}/preview", post(preview_frame))
        .route(
            "/sources/{id}/immich/credentials",
            post(set_immich_credentials),
        )
        .route("/sources/{id}/immich/filters", post(set_immich_filters))
        .route("/sources/{id}/refresh", post(refresh_source))
        .route("/sources/reload", post(reload_sources))
        .with_state(state.clone())
        .layer(cors)
        .layer(trace)
        .layer(middleware::from_fn(log_error_responses));

    let app = Router::new().nest("/api", api);

    #[cfg(feature = "embed_ui")]
    let app = app
        .route("/", get(ui::serve_ui))
        .route("/{*path}", get(ui::serve_ui));

    app
}

/// Clear the device screen to solid white by pushing a white PNG the size of the panel.
pub async fn clear_frame(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let cfg = config::ConfigManager::to_struct(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let frame_cfg = cfg
        .photoframes
        .get(&frame_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let (w, h) = match (frame_cfg.panel_width, frame_cfg.panel_height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let pixels = vec![255u8; (w as usize) * (h as usize) * 4];
    let prepared = crate::frame::PreparedFrameImage {
        width: w,
        height: h,
        pixels,
    };
    if crate::frame::push_to_device(&frame_id, frame_cfg, &prepared)
        .await
        .is_err()
    {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let _ = crate::frame::save_prepared(&frame_id, &prepared);
    Ok(StatusCode::ACCEPTED)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePaletteEntry {
    pub input: String,
    pub hex: String,
    pub rgb: [u8; 3],
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePaletteResponse {
    pub frame_id: String,
    pub palette: Vec<FramePaletteEntry>,
}

pub async fn frame_palette(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<FramePaletteResponse>, StatusCode> {
    let cfg = config::ConfigManager::to_struct(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let frame = cfg
        .photoframes
        .get(&frame_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let mut out = Vec::new();
    for s in &frame.supported_colors {
        if let Ok(parsed) = css_color::Srgb::from_str(s) {
            let r = (parsed.red * 255.0).round().clamp(0.0, 255.0) as u8;
            let g = (parsed.green * 255.0).round().clamp(0.0, 255.0) as u8;
            let b = (parsed.blue * 255.0).round().clamp(0.0, 255.0) as u8;
            let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
            out.push(FramePaletteEntry {
                input: s.clone(),
                hex,
                rgb: [r, g, b],
            });
        } else {
            out.push(FramePaletteEntry {
                input: s.clone(),
                hex: String::from("invalid"),
                rgb: [0, 0, 0],
            });
        }
    }
    Ok(Json(FramePaletteResponse {
        frame_id,
        palette: out,
    }))
}

pub async fn upload_frame(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, StatusCode> {
    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.name().map(|s| s.to_string());
        if name.as_deref() == Some("file") {
            data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| StatusCode::BAD_REQUEST)?
                    .to_vec(),
            );
            break;
        }
    }
    let data = data.ok_or(StatusCode::BAD_REQUEST)?;
    let cfg = config::ConfigManager::to_struct(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let frame_cfg = cfg
        .photoframes
        .get(&frame_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let limits = cfg.image_limits.as_ref();
    match crate::frame::handle_direct_upload(&frame_id, frame_cfg, &data, limits).await {
        Ok(prepared) => {
            if let Err(e) = crate::frame::save_prepared(&frame_id, &prepared) {
                tracing::warn!(frame = %frame_id, error = %e, "saving uploaded file failed");
            }
            Ok(Json(UploadResponse {
                frame_id,
                width: prepared.width,
                height: prepared.height,
            }))
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

#[derive(Deserialize)]
pub struct ImmichCredsPayload {
    pub base_url: String,
    pub api_key: String,
}

#[instrument(err, skip_all)]
pub async fn set_immich_credentials(
    Path(source_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<ImmichCredsPayload>,
) -> Result<StatusCode, StatusCode> {
    config::ConfigManager::set_immich_credentials(
        &state.cfg,
        &source_id,
        &payload.base_url,
        &payload.api_key,
    )
    .await
    .map_err(|_| StatusCode::BAD_REQUEST)?;
    config::ConfigManager::save(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Reload sources to pick up the new credentials
    state
        .scheduler
        .reload_sources()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct ImmichFiltersPayload {
    pub filters: JsonValue,
}

#[instrument(err, skip_all)]
pub async fn set_immich_filters(
    Path(source_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<ImmichFiltersPayload>,
) -> Result<StatusCode, StatusCode> {
    config::ConfigManager::set_immich_filters(&state.cfg, &source_id, &payload.filters)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    config::ConfigManager::save(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Reload sources to pick up the new filters
    state
        .scheduler
        .reload_sources()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

/// Serve the HTTP API. If `bind` is Some it is parsed as a socket address, otherwise
/// defaults to 0.0.0.0:8080.
pub async fn serve(app: Router, bind: Option<String>) -> anyhow::Result<()> {
    let bind_addr = bind.unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let addr: SocketAddr = bind_addr.parse()?;
    tracing::info!(addr=%addr, "starting http server");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

pub async fn preview_frame(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
    maybe_payload: Option<Json<FrameUpdate>>,
) -> Result<Response, StatusCode> {
    if frame_id.contains('/') || frame_id.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let cfg_now = config::ConfigManager::to_struct(&state.cfg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let frame_cfg = cfg_now
        .photoframes
        .get(&frame_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let Some(base) = frame::get_base_image(&frame_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::NOT_FOUND);
    };
    let mut effective = frame_cfg.clone();
    if let Some(ref payload) = maybe_payload {
        if payload.brightness.is_some()
            || payload.contrast.is_some()
            || payload.saturation.is_some()
            || payload.sharpness.is_some()
        {
            let mut a = effective.adjustments.clone().unwrap_or_default();
            if let Some(v) = payload.brightness {
                a.brightness = v;
            }
            if let Some(v) = payload.contrast {
                a.contrast = v;
            }
            if let Some(v) = payload.saturation {
                a.saturation = v;
            }
            if let Some(v) = payload.sharpness {
                a.sharpness = v;
            }
            effective.adjustments = Some(a);
        }
        if let Some(ref d) = payload.dithering {
            effective.dithering = Some(d.clone());
        }
        if payload.left.is_some()
            || payload.right.is_some()
            || payload.top.is_some()
            || payload.bottom.is_some()
        {
            let mut o = effective.overscan.clone().unwrap_or_default();
            if let Some(v) = payload.left {
                o.left = v;
            }
            if let Some(v) = payload.right {
                o.right = v;
            }
            if let Some(v) = payload.top {
                o.top = v;
            }
            if let Some(v) = payload.bottom {
                o.bottom = v;
            }
            effective.overscan = Some(o);
        }
    }
    let prepared = frame::prepare_from_base(&effective, &base);
    let img = image::RgbaImage::from_raw(prepared.width, prepared.height, prepared.pixels)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let dynimg = image::DynamicImage::ImageRgba8(img);
    let mut png_bytes = Vec::new();
    dynimg
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(header::CONTENT_TYPE, "image/png")], png_bytes).into_response())
}

/// Return the last saved intermediate PNG (pre-dither/adjust), or 404 if missing.
pub async fn get_intermediate_image(Path(frame_id): Path<String>) -> Result<Response, StatusCode> {
    if frame_id.contains('/') || frame_id.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let path = std::path::PathBuf::from(format!("{frame_id}_intermediate.png"));
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}

pub async fn refresh_source(
    Path(source_id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    state
        .scheduler
        .refresh_source(&source_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::ACCEPTED)
}

#[instrument(err, skip_all)]
pub async fn reload_sources(State(state): State<AppState>) -> Result<StatusCode, StatusCode> {
    state
        .scheduler
        .reload_sources()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

pub async fn trigger_frame(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    // First try to push currently cached base (e.g., just uploaded or primed); if none, run full selection.
    if let Err(e) = state.scheduler.push_cached_base(&frame_id).await {
        tracing::debug!(frame=%frame_id, error=%e, "push_cached_base failed; falling back to trigger");
        state
            .scheduler
            .trigger_frame(&frame_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(StatusCode::ACCEPTED)
}

pub async fn next_frame(
    Path(frame_id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    state
        .scheduler
        .prime_next_image(&frame_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::ACCEPTED)
}
