use crate::stats::ServiceStats;
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::Response,
    routing::any,
};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
    sync::oneshot,
};
use tokio_util::io::ReaderStream;

pub struct RunningServer {
    pub port: u16,
    pub shutdown: oneshot::Sender<()>,
}

#[derive(Clone)]
struct StaticContext {
    root: PathBuf,
    canonical_root: PathBuf,
    root_file: Option<PathBuf>,
    spa_fallback: bool,
    directory_listing: bool,
    stats: Arc<ServiceStats>,
}

pub async fn start(
    root: PathBuf,
    spa_fallback: bool,
    directory_listing: bool,
    stats: Arc<ServiceStats>,
) -> Result<RunningServer, String> {
    let root = tokio::fs::canonicalize(&root)
        .await
        .map_err(|error| format!("Could not open the static folder: {error}"))?;
    let metadata = tokio::fs::metadata(&root)
        .await
        .map_err(|error| format!("Could not read the static path: {error}"))?;

    if !metadata.is_file() && !metadata.is_dir() {
        return Err("The static path must be a file or a folder.".to_string());
    }

    let root_is_file = metadata.is_file();
    let canonical_root = if root_is_file {
        root.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| root.clone())
    } else {
        root.clone()
    };
    let root_file = root_is_file.then_some(root.clone());

    let context = StaticContext {
        root,
        canonical_root,
        root_file,
        spa_fallback,
        directory_listing,
        stats,
    };

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| format!("Could not bind the static server: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the static server address: {error}"))?
        .port();

    let router = Router::new().fallback(any(handle)).with_state(context);
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

async fn handle(State(context): State<StaticContext>, request: Request) -> Response {
    let method = request.method().clone();
    if method != Method::GET && method != Method::HEAD {
        return simple_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "text/plain; charset=utf-8",
            "Method not allowed.",
        )
        .with_header(header::ALLOW, "GET, HEAD");
    }

    context.stats.request(0);

    let segments = match path_segments(request.uri().path()) {
        Ok(segments) => segments,
        Err(()) => return not_found(&context),
    };

    if let Some(root_file) = &context.root_file {
        if !segments.is_empty() {
            return not_found(&context);
        }
        return serve_file(
            &context,
            root_file.clone(),
            &method,
            request.headers().get(header::RANGE),
        )
        .await;
    }

    let target = context.root.join(segments.join("/"));
    let target = match tokio::fs::canonicalize(&target).await {
        Ok(path) if path.starts_with(&context.canonical_root) => path,
        _ if context.spa_fallback => {
            return serve_spa(&context, &method, request.headers().get(header::RANGE)).await;
        }
        _ => return not_found(&context),
    };

    match tokio::fs::metadata(&target).await {
        Ok(metadata) if metadata.is_dir() => {
            let index = target.join("index.html");
            if tokio::fs::metadata(&index).await.is_ok() {
                return serve_file(
                    &context,
                    index,
                    &method,
                    request.headers().get(header::RANGE),
                )
                .await;
            }
            if context.directory_listing {
                return directory_listing(&context, &target).await;
            }
            if context.spa_fallback {
                return serve_spa(&context, &method, request.headers().get(header::RANGE)).await;
            }
            not_found(&context)
        }
        Ok(metadata) if metadata.is_file() => {
            serve_file(
                &context,
                target,
                &method,
                request.headers().get(header::RANGE),
            )
            .await
        }
        _ if context.spa_fallback => {
            serve_spa(&context, &method, request.headers().get(header::RANGE)).await
        }
        _ => not_found(&context),
    }
}

async fn serve_spa(
    context: &StaticContext,
    method: &Method,
    range: Option<&HeaderValue>,
) -> Response {
    let index = context.root.join("index.html");
    if tokio::fs::metadata(&index).await.is_ok() {
        serve_file(context, index, method, range).await
    } else {
        not_found(context)
    }
}

async fn serve_file(
    context: &StaticContext,
    path: PathBuf,
    method: &Method,
    range_header: Option<&HeaderValue>,
) -> Response {
    let canonical = match tokio::fs::canonicalize(&path).await {
        Ok(path) => path,
        Err(_) => return not_found(context),
    };
    if !canonical.starts_with(&context.canonical_root) {
        return not_found(context);
    }

    let metadata = match tokio::fs::metadata(&canonical).await {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return not_found(context),
    };
    let length = metadata.len();
    let content_type = mime_guess::from_path(&canonical)
        .first_or_octet_stream()
        .to_string();

    let range = range_header.and_then(|value| value.to_str().ok());
    let parsed_range = match range {
        Some(value) => match parse_range(value, length) {
            Ok(range) => range,
            Err(()) => {
                return simple_response(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    "text/plain; charset=utf-8",
                    "Requested range not satisfiable.",
                )
                .with_header(header::CONTENT_RANGE, format!("bytes */{length}"));
            }
        },
        None => None,
    };

    let (status, start, end) = match parsed_range {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
        None => (StatusCode::OK, 0, length.saturating_sub(1)),
    };
    let content_length = if length == 0 {
        0
    } else {
        end.saturating_sub(start) + 1
    };

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header("x-content-type-options", "nosniff")
        .header(header::CACHE_CONTROL, "no-cache");

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{length}"),
        );
    }

    if method == Method::HEAD {
        return builder.body(Body::empty()).unwrap_or_else(|_| {
            simple_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/plain; charset=utf-8",
                "Could not build the response.",
            )
        });
    }

    let mut file = match tokio::fs::File::open(&canonical).await {
        Ok(file) => file,
        Err(_) => return not_found(context),
    };
    if status == StatusCode::PARTIAL_CONTENT && file.seek(SeekFrom::Start(start)).await.is_err() {
        return not_found(context);
    }

    context.stats.add_bytes_out(content_length);
    let body = Body::from_stream(ReaderStream::new(file.take(content_length)));

    builder.body(body).unwrap_or_else(|_| {
        simple_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "text/plain; charset=utf-8",
            "Could not build the response.",
        )
    })
}

async fn directory_listing(context: &StaticContext, path: &Path) -> Response {
    let mut entries = match tokio::fs::read_dir(path).await {
        Ok(entries) => entries,
        Err(_) => return not_found(context),
    };

    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry
            .file_type()
            .await
            .map(|kind| kind.is_dir())
            .unwrap_or(false);
        names.push((name, is_dir));
    }
    names.sort_by_key(|left| left.0.to_lowercase());

    let mut items = String::new();
    for (name, is_dir) in names {
        let href = utf8_percent_encode(&name, NON_ALPHANUMERIC).to_string();
        let label = escape_html(&name);
        let suffix = if is_dir { "/" } else { "" };
        items.push_str(&format!(
            "<li><a href=\"{href}{suffix}\">{label}{suffix}</a></li>"
        ));
    }

    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Index of /</title><style>\
         :root{{color-scheme:dark}}*{{box-sizing:border-box}}\
         body{{margin:0;background:#0a0a0a;color:#e8e8e8;font:14px/1.6 ui-monospace,SFMono-Regular,Menlo,monospace}}\
         main{{max-width:760px;margin:8vh auto;padding:0 24px}}\
         h1{{font-size:15px;font-weight:500;border-bottom:1px solid #292929;padding-bottom:14px}}\
         ul{{list-style:none;padding:0}}li{{border-bottom:1px solid #1b1b1b}}\
         a{{display:block;padding:10px 0;color:#e8e8e8;text-decoration:none}}\
         a:hover{{color:#fff;text-decoration:underline}}small{{color:#777}}\
         </style></head><body><main><h1>Index of /</h1><ul>{items}</ul>\
         <p><small>Served by OnionHoster</small></p></main></body></html>"
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(html))
        .unwrap_or_else(|_| {
            simple_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/plain; charset=utf-8",
                "Could not build the response.",
            )
        })
}

fn path_segments(path: &str) -> Result<Vec<String>, ()> {
    let decoded = percent_decode_str(path).decode_utf8().map_err(|_| ())?;
    let mut segments = Vec::new();

    for segment in decoded.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.starts_with('.') {
            return Err(());
        }
        if segment.contains('\\') || segment.contains('\0') {
            return Err(());
        }
        segments.push(segment.to_string());
    }

    Ok(segments)
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

fn not_found(_context: &StaticContext) -> Response {
    let html = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
        <title>Not found</title><style>\
        :root{color-scheme:dark}body{margin:0;background:#0a0a0a;color:#e8e8e8;\
        font:14px/1.6 ui-monospace,SFMono-Regular,Menlo,monospace}\
        main{max-width:560px;margin:14vh auto;padding:0 24px}\
        h1{font-size:15px;font-weight:500}p{color:#8a8a8a}\
        </style></head><body><main><h1>404</h1>\
        <p>The requested file was not found on this onion service.</p>\
        </main></body></html>";

    simple_response(StatusCode::NOT_FOUND, "text/html; charset=utf-8", html)
}

fn simple_response(status: StatusCode, content_type: &str, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header("x-content-type-options", "nosniff")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

trait ResponseHeaderExt {
    fn with_header(self, name: header::HeaderName, value: impl AsRef<str>) -> Response;
}

impl ResponseHeaderExt for Response {
    fn with_header(mut self, name: header::HeaderName, value: impl AsRef<str>) -> Response {
        if let Ok(value) = HeaderValue::from_str(value.as_ref()) {
            self.headers_mut().insert(name, value);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_and_dotfiles() {
        assert_eq!(path_segments("/"), Ok(Vec::<String>::new()));
        assert_eq!(
            path_segments("/assets/app.js"),
            Ok(vec!["assets".to_string(), "app.js".to_string()])
        );
        assert!(path_segments("/../secret").is_err());
        assert!(path_segments("/%2e%2e/secret").is_err());
        assert!(path_segments("/.git/config").is_err());
        assert!(path_segments("/assets/%00.js").is_err());
    }

    #[test]
    fn parses_range_headers() {
        assert_eq!(parse_range("bytes=0-9", 100), Ok(Some((0, 9))));
        assert_eq!(parse_range("bytes=90-", 100), Ok(Some((90, 99))));
        assert_eq!(parse_range("bytes=-10", 100), Ok(Some((90, 99))));
        assert_eq!(parse_range("bytes=0-999", 100), Ok(Some((0, 99))));
        assert_eq!(parse_range("items=0-1", 100), Ok(None));
        assert_eq!(parse_range("bytes=100-", 100), Err(()));
        assert_eq!(parse_range("bytes=20-10", 100), Err(()));
        assert_eq!(parse_range("bytes=0-1,3-4", 100), Err(()));
    }
}
