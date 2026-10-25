use crate::stats::ServiceStats;
use axum::{
    body::Body,
    http::{Method, StatusCode, header},
    response::Response,
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use rusqlite::Connection;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
    net::TcpListener,
    sync::oneshot,
};
use tokio_util::io::ReaderStream;

pub const CHAT_MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
pub const CHAT_QUOTA_BYTES: u64 = 1024 * 1024 * 1024;
pub const FILE_MAX_BYTES: u64 = 50 * 1024 * 1024;
pub const FILE_QUOTA_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub const LOGO_SVG: &str = include_str!("../../app-icon.svg");

pub const CHAT_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src 'self' data:; media-src 'self'; connect-src 'self' ws: wss:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
pub const FILES_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src 'self' data:; media-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub struct RunningServer {
    pub port: u16,
    pub shutdown: oneshot::Sender<()>,
}

pub fn prepare_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|error| {
        format!(
            "Could not create the community data directory at {}: {error}",
            path.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                format!(
                    "Could not protect the community data directory at {}: {error}",
                    path.display()
                )
            },
        )?;
    }

    Ok(())
}

pub fn open_database(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path)
        .map_err(|error| format!("Could not open the community database: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Could not configure the community database: {error}"))?;
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL;\nPRAGMA synchronous=NORMAL;\nPRAGMA foreign_keys=ON;",
        )
        .map_err(|error| format!("Could not initialize the community database: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(connection)
}

pub async fn spawn_router(router: axum::Router) -> Result<RunningServer, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| format!("Could not bind the community server: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the community server address: {error}"))?
        .port();

    let (shutdown, receiver) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = receiver.await;
            })
            .await;
    });

    Ok(RunningServer { port, shutdown })
}

pub fn client_id_valid(value: &str) -> bool {
    (8..=80).contains(&value.len())
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

pub fn identity_for(client_id: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in client_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("anonymous#{:04}", 1000 + (hash % 9000))
}

pub fn clean_name(value: Option<&str>) -> Option<String> {
    let value = value?;
    let value: String = value
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(32).collect())
}

pub fn author_for(client_id: &str, display_name: Option<&str>) -> String {
    display_name
        .map(str::to_string)
        .unwrap_or_else(|| identity_for(client_id))
}

pub fn clean_body(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.chars().count() > 2000 {
        return Err("Messages can be at most 2000 characters.".to_string());
    }
    Ok(value.to_string())
}

pub fn safe_file_name(value: &str) -> String {
    let base = value.rsplit(['/', '\\']).next().unwrap_or("file");
    let cleaned: String = base
        .chars()
        .filter(|character| !character.is_control() && *character != '"')
        .take(160)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned.to_string()
    }
}

pub fn mime_for_name(name: &str) -> String {
    let mime = mime_guess::from_path(name)
        .first_raw()
        .unwrap_or("application/octet-stream");
    if mime == "image/svg+xml" || mime.starts_with("text/html") {
        "application/octet-stream".to_string()
    } else {
        mime.to_string()
    }
}

pub fn media_kind(mime: &str) -> &'static str {
    if mime.starts_with("image/") && mime != "image/svg+xml" {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else {
        "file"
    }
}

pub fn page_response(
    stats: &ServiceStats,
    method: &Method,
    status: StatusCode,
    body: &'static str,
    csp: &'static str,
) -> Response {
    stats.request(0);

    let builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .header(header::CACHE_CONTROL, "no-store")
        .header("content-security-policy", csp)
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "DENY");

    if method == Method::HEAD {
        return builder.body(Body::empty()).unwrap_or_else(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not build the response.",
            )
        });
    }

    stats.add_bytes_out(body.len() as u64);
    builder.body(Body::from(body)).unwrap_or_else(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not build the response.",
        )
    })
}

pub fn logo_response(stats: &ServiceStats, method: &Method) -> Response {
    stats.request(0);

    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")
        .header(header::CONTENT_LENGTH, LOGO_SVG.len().to_string())
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .header(
            "content-security-policy",
            "default-src 'none'; style-src 'unsafe-inline'; img-src 'self' data:",
        )
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "DENY");

    if method == Method::HEAD {
        return builder.body(Body::empty()).unwrap_or_else(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not build the response.",
            )
        });
    }

    stats.add_bytes_out(LOGO_SVG.len() as u64);
    builder.body(Body::from(LOGO_SVG)).unwrap_or_else(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not build the response.",
        )
    })
}

pub fn json_response<T: serde::Serialize>(
    stats: &ServiceStats,
    status: StatusCode,
    value: &T,
) -> Response {
    match serde_json::to_vec(value) {
        Ok(bytes) => {
            stats.add_bytes_out(bytes.len() as u64);
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::CONTENT_LENGTH, bytes.len().to_string())
                .header(header::CACHE_CONTROL, "no-store")
                .header("x-content-type-options", "nosniff")
                .body(Body::from(bytes))
                .unwrap_or_else(|_| {
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Could not build the response.",
                    )
                })
        }
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not encode the response.",
        ),
    }
}

pub fn error_response(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(message.to_string()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

#[allow(clippy::too_many_arguments)]
pub async fn file_response(
    stats: &ServiceStats,
    path: &Path,
    file_name: &str,
    mime: &str,
    size: u64,
    range: Option<&str>,
    download: bool,
    method: &Method,
) -> Response {
    stats.request(0);
    let inline = !download && media_kind(mime) != "file";
    let content_type = if inline {
        mime.to_string()
    } else {
        "application/octet-stream".to_string()
    };

    let parsed_range = match range {
        Some(value) => match parse_range(value, size) {
            Ok(range) => range,
            Err(()) => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                    .header("x-content-type-options", "nosniff")
                    .body(Body::from("Requested range not satisfiable."))
                    .unwrap_or_else(|_| {
                        error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Could not build the response.",
                        )
                    });
            }
        },
        None => None,
    };

    let (status, start, end) = match parsed_range {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
        None => (StatusCode::OK, 0, size.saturating_sub(1)),
    };
    let content_length = if size == 0 {
        0
    } else {
        end.saturating_sub(start) + 1
    };

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(
            header::CONTENT_DISPOSITION,
            content_disposition(file_name, inline),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "DENY");

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }

    if method == Method::HEAD {
        return builder.body(Body::empty()).unwrap_or_else(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not build the response.",
            )
        });
    }

    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return error_response(StatusCode::NOT_FOUND, "The file was not found."),
    };
    if status == StatusCode::PARTIAL_CONTENT && file.seek(SeekFrom::Start(start)).await.is_err() {
        return error_response(StatusCode::NOT_FOUND, "The file was not found.");
    }

    stats.add_bytes_out(content_length);
    let body = Body::from_stream(ReaderStream::new(file.take(content_length)));
    builder.body(body).unwrap_or_else(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not build the response.",
        )
    })
}

pub fn remove_data_dir(base: &Path) -> Result<(), String> {
    if !base.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(base).map_err(|error| {
        format!(
            "Could not remove the community data at {}: {error}",
            base.display()
        )
    })
}

fn content_disposition(name: &str, inline: bool) -> String {
    let fallback: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    let fallback = if fallback.trim().is_empty() {
        "file".to_string()
    } else {
        fallback
    };
    let encoded = utf8_percent_encode(name, NON_ALPHANUMERIC).to_string();
    format!(
        "{}; filename=\"{}\"; filename*=UTF-8''{}",
        if inline { "inline" } else { "attachment" },
        fallback,
        encoded
    )
}

fn parse_range(value: &str, length: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(spec) = value.strip_prefix("bytes=") else {
        return Ok(None);
    };
    if spec.contains(',') || length == 0 {
        return Err(());
    }

    let (start_text, end_text) = spec.split_once('-').ok_or(())?;
    if start_text.is_empty() {
        let suffix = end_text.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        return Ok(Some((length.saturating_sub(suffix), length - 1)));
    }

    let start = start_text.parse::<u64>().map_err(|_| ())?;
    if start >= length {
        return Err(());
    }
    let end = if end_text.is_empty() {
        length - 1
    } else {
        end_text.parse::<u64>().map_err(|_| ())?.min(length - 1)
    };
    if end < start {
        return Err(());
    }

    Ok(Some((start, end)))
}

pub fn data_paths(base: PathBuf) -> Result<(PathBuf, PathBuf), String> {
    prepare_dir(&base)?;
    let files = base.join("files");
    prepare_dir(&files)?;
    let database = base.join("community.sqlite3");
    Ok((files, database))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_tags_are_stable_and_four_digits() {
        let first = identity_for("0f6d3f1e-4dfb-4aa7-9d3a-1fdb5b8d2c11");
        let second = identity_for("0f6d3f1e-4dfb-4aa7-9d3a-1fdb5b8d2c11");
        assert_eq!(first, second);
        assert!(first.starts_with("anonymous#"));
        let digits = first.trim_start_matches("anonymous#");
        assert_eq!(digits.len(), 4);
        assert!(digits.chars().all(|character| character.is_ascii_digit()));
    }

    #[test]
    fn file_names_are_safe_for_headers() {
        assert_eq!(safe_file_name("../../secret.txt"), "secret.txt");
        assert_eq!(safe_file_name("hello\"world.txt"), "helloworld.txt");
        assert_eq!(safe_file_name(""), "file");
    }

    #[test]
    fn only_browser_media_is_inline() {
        assert_eq!(media_kind("image/png"), "image");
        assert_eq!(media_kind("video/mp4"), "video");
        assert_eq!(media_kind("image/svg+xml"), "file");
        assert_eq!(media_kind("text/html"), "file");
    }

    #[test]
    fn parses_byte_ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Ok(Some((0, 99))));
        assert_eq!(parse_range("bytes=-100", 1000), Ok(Some((900, 999))));
        assert_eq!(parse_range("bytes=500-", 1000), Ok(Some((500, 999))));
        assert_eq!(parse_range("bytes=1000-", 1000), Err(()));
    }
}
