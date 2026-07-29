//! HTTP delivery via actix-web: serves the manifests and segments with correct
//! content types, permissive CORS, and no-cache on the live manifests.

use std::path::{Component, Path, PathBuf};

use actix_cors::Cors;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{App, HttpResponse, HttpServer, web};

use crate::config::Config;

struct AppState {
    dir: PathBuf,
    hls: bool,
    dash: bool,
}

pub async fn serve(config: Config) -> std::io::Result<()> {
    let state = web::Data::new(AppState {
        dir: config.output.dir.clone(),
        hls: config.hls.enabled,
        dash: config.dash.enabled,
    });
    let bind = (config.server.host.clone(), config.server.port);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Cors::permissive())
            .route("/", web::get().to(index))
            .route("/{path:.*}", web::get().to(serve_file))
    })
    .bind(bind)?
    .run()
    .await
}

async fn index(state: web::Data<AppState>) -> HttpResponse {
    let mut links = String::new();
    if state.hls {
        links.push_str(
            "<li><a href=\"/stream.m3u8\">/stream.m3u8</a> (HLS media playlist)</li>\
             <li><a href=\"/master.m3u8\">/master.m3u8</a> (HLS multivariant)</li>",
        );
    }
    if state.dash {
        links.push_str("<li><a href=\"/stream.mpd\">/stream.mpd</a> (MPEG-DASH)</li>");
    }
    let audio = if state.hls {
        "<audio controls src=\"/stream.m3u8\"></audio>\
         <p><small>Native playback works in Safari; other browsers need an HLS/DASH player.</small></p>"
    } else {
        ""
    };
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>hlsd</title></head>\
         <body><h1>hlsd</h1><ul>{links}</ul>{audio}</body></html>"
    );
    HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, "text/html; charset=utf-8"))
        .body(body)
}

async fn serve_file(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let rel = path.into_inner();
    let Some(safe) = sanitize(&rel) else {
        return HttpResponse::BadRequest().body("invalid path");
    };
    let full = state.dir.join(&safe);

    match tokio::fs::read(&full).await {
        Ok(bytes) => {
            let (ctype, cache) = content_meta(&full);
            HttpResponse::Ok()
                .insert_header((CONTENT_TYPE, ctype))
                .insert_header((CACHE_CONTROL, cache))
                .body(bytes)
        }
        Err(_) => HttpResponse::NotFound().body("not found"),
    }
}

/// Reject path traversal; return a relative path with only normal components.
fn sanitize(rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            // Anything else (`..`, root, prefix) is rejected outright.
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        return None;
    }
    Some(out)
}

fn content_meta(path: &Path) -> (&'static str, &'static str) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "m3u8" => ("application/vnd.apple.mpegurl", "no-cache"),
        "mpd" => ("application/dash+xml", "no-cache"),
        // Init segment: immutable, cache it.
        "mp4" => ("video/mp4", "public, max-age=31536000, immutable"),
        // Media segments: cacheable for the window lifetime.
        "m4s" => ("video/iso.segment", "public, max-age=3600"),
        _ => ("application/octet-stream", "no-cache"),
    }
}
