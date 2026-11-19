//! The dashboard (built from ui/) embedded into the controller binary.
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/dist"]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path) {
        Some(f) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let cache = if path == "index.html" { "no-cache" } else { "public, max-age=31536000, immutable" };
            ([(header::CONTENT_TYPE, mime.as_ref().to_string()), (header::CACHE_CONTROL, cache.to_string())], f.data.into_owned()).into_response()
        }
        None => {
            // single-page app: unknown non-API paths load the shell
            if path.starts_with("api/") || path.contains('.') {
                return StatusCode::NOT_FOUND.into_response();
            }
            match Assets::get("index.html") {
                Some(f) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()), (header::CACHE_CONTROL, "no-cache".to_string())], f.data.into_owned()).into_response(),
                None => (StatusCode::NOT_FOUND, "dashboard not built").into_response(),
            }
        }
    }
}
