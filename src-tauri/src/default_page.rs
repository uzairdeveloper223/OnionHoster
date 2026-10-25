use crate::stats::ServiceStats;
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Method, StatusCode, header},
    response::Response,
    routing::get,
};
use std::sync::Arc;
use tokio::sync::oneshot;

const PAGE: &str = include_str!("../assets/default-page.html");
const ICON: &str = include_str!("../../app-icon.svg");
const ROBOTS: &str = "User-agent: *\nAllow: /\n";
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const NOT_FOUND_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Not found - OnionHoster</title>
<style>
:root{color-scheme:light dark;--bg:#f6f6f6;--ink:#0a0a0a;--muted:#5f5f5f;--line:#dedede}
@media (prefers-color-scheme:dark){:root{--bg:#080808;--ink:#f5f5f5;--muted:#a3a3a3;--line:#232323}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.7 ui-sans-serif,system-ui,sans-serif}
main{max-width:560px;margin:18vh auto 0;padding:0 24px}
p{margin:0 0 14px;color:var(--muted)}
a{color:var(--ink);text-underline-offset:3px}
h1{margin:0 0 14px;font-size:30px;line-height:1.2;letter-spacing:-.03em}
code{font:11px ui-monospace,SFMono-Regular,Menlo,monospace;color:var(--muted)}
</style>
</head>
<body>
<main>
<h1>Page not found.</h1>
<p>This service only serves the built-in OnionHoster page.</p>
<p><a href="/">Go to the start page</a></p>
<p><code>Served by OnionHoster</code></p>
</main>
</body>
</html>"#;

pub struct RunningServer {
    pub port: u16,
    pub shutdown: oneshot::Sender<()>,
}

#[derive(Clone)]
struct PageContext {
    stats: Arc<ServiceStats>,
}

pub async fn start(stats: Arc<ServiceStats>) -> Result<RunningServer, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| format!("Could not bind the default page server: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the default page server address: {error}"))?
        .port();

    let router = Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/logo.svg", get(icon))
        .route("/favicon.svg", get(icon))
        .route("/favicon.ico", get(favicon_ico))
        .route("/robots.txt", get(robots))
        .fallback(not_found)
        .with_state(PageContext { stats });

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

async fn index(State(context): State<PageContext>, method: Method) -> Response {
    page_response(
        &context,
        &method,
        StatusCode::OK,
        "text/html; charset=utf-8",
        PAGE,
        "no-store",
    )
}

async fn icon(State(context): State<PageContext>, method: Method) -> Response {
    page_response(
        &context,
        &method,
        StatusCode::OK,
        "image/svg+xml; charset=utf-8",
        ICON,
        "public, max-age=3600",
    )
}

async fn robots(State(context): State<PageContext>, method: Method) -> Response {
    page_response(
        &context,
        &method,
        StatusCode::OK,
        "text/plain; charset=utf-8",
        ROBOTS,
        "public, max-age=3600",
    )
}

async fn favicon_ico() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn not_found(State(context): State<PageContext>, method: Method) -> Response {
    page_response(
        &context,
        &method,
        StatusCode::NOT_FOUND,
        "text/html; charset=utf-8",
        NOT_FOUND_PAGE,
        "no-store",
    )
}

fn page_response(
    context: &PageContext,
    method: &Method,
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
    cache_control: &'static str,
) -> Response {
    context.stats.request(0);

    let builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .header(header::CACHE_CONTROL, cache_control)
        .header("content-security-policy", CONTENT_SECURITY_POLICY)
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "DENY");

    if method == Method::HEAD {
        return builder
            .body(Body::empty())
            .unwrap_or_else(|_| internal_error());
    }

    context.stats.add_bytes_out(body.len() as u64);
    builder
        .body(Body::from(body))
        .unwrap_or_else(|_| internal_error())
}

fn internal_error() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header("x-content-type-options", "nosniff")
        .body(Body::from("Could not build the response."))
        .expect("static response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn default_page_contains_project_details() {
        assert!(PAGE.contains("This site is running through OnionHoster."));
        assert!(PAGE.contains("rel=\"icon\" href=\"/favicon.svg\""));
        assert!(PAGE.contains("src=\"/logo.svg\""));
        assert!(PAGE.contains("https://github.com/uzairdeveloper223/OnionHoster"));
        assert!(PAGE.contains("https://www.linkedin.com/in/uzairmughal001/"));
        assert!(PAGE.contains("GPL-3.0-or-later"));
        assert!(PAGE.contains("not affiliated with the Tor Project"));
        assert!(ICON.contains("<svg"));
    }

    #[tokio::test]
    async fn serves_the_page_and_icon() {
        let stats = Arc::new(ServiceStats::default());
        let server = start(stats.clone())
            .await
            .expect("start default page server");

        let page = request(
            server.port,
            "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(
            page.to_ascii_lowercase()
                .contains("content-type: text/html")
        );
        assert!(page.contains("This site is running through OnionHoster."));

        let icon = request(
            server.port,
            "GET /favicon.svg HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(icon.starts_with("HTTP/1.1 200"));
        assert!(
            icon.to_ascii_lowercase()
                .contains("content-type: image/svg+xml")
        );
        assert!(icon.contains("<svg"));

        assert_eq!(stats.requests.load(Ordering::Relaxed), 2);
        let _ = server.shutdown.send(());
    }

    async fn request(port: u16, request: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect to default page server");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write request");
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.expect("read response");
        String::from_utf8_lossy(&bytes).into_owned()
    }
}
