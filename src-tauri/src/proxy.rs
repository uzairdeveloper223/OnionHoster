use crate::services::ProxyScheme;
use crate::stats::ServiceStats;
use axum::{
    Router,
    body::Body,
    extract::{
        FromRequestParts, Request, State, WebSocketUpgrade,
        ws::{Message as ClientMessage, WebSocket},
    },
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, Version, header,
        request::Parts,
    },
    response::{IntoResponse, Response},
    routing::any,
};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;
use tokio_tungstenite::{Connector as WebSocketConnector, tungstenite::Message as UpstreamMessage};

type ProxyClient = Client<hyper_rustls::HttpsConnector<HttpConnector>, Body>;

pub struct RunningServer {
    pub port: u16,
    pub shutdown: oneshot::Sender<()>,
}

#[derive(Clone)]
struct ProxyContext {
    scheme: ProxyScheme,
    host: String,
    port: u16,
    rewrite_host: bool,
    client: ProxyClient,
    ws_tls: Arc<rustls::ClientConfig>,
    stats: Arc<ServiceStats>,
}

#[derive(Debug)]
struct AcceptAnyServerCert {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn tls_config(accept_invalid_certs: bool) -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    if accept_invalid_certs {
        let provider = config.crypto_provider().clone();
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(AcceptAnyServerCert { provider }));
    }

    config
}

impl ProxyContext {
    fn authority(&self) -> String {
        let host = if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let default_port = match self.scheme {
            ProxyScheme::Http => self.port == 80,
            ProxyScheme::Https => self.port == 443,
        };
        if default_port {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }

    fn origin(&self) -> String {
        format!("{}://{}", self.scheme.as_str(), self.authority())
    }
}

pub async fn start(
    scheme: ProxyScheme,
    host: String,
    port: u16,
    rewrite_host: bool,
    accept_invalid_certs: bool,
    stats: Arc<ServiceStats>,
) -> Result<RunningServer, String> {
    let tls = tls_config(accept_invalid_certs);
    let ws_tls = Arc::new(tls.clone());
    let connector = HttpsConnectorBuilder::new()
        .with_tls_config(tls)
        .https_or_http()
        .enable_http1()
        .build();
    let client = Client::builder(TokioExecutor::new()).build(connector);

    let context = ProxyContext {
        scheme,
        host,
        port,
        rewrite_host,
        client,
        ws_tls,
        stats,
    };

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| format!("Could not bind the local proxy: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the local proxy address: {error}"))?
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

async fn handle(State(context): State<ProxyContext>, request: Request) -> Response {
    let (parts, body) = request.into_parts();

    if is_websocket(&parts.headers) {
        return handle_websocket(context, parts, body).await;
    }

    context.stats.request(0);

    let path = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let uri = match format!("{}{}", context.origin(), path).parse::<Uri>() {
        Ok(uri) => uri,
        Err(_) => return bad_gateway("The upstream address is invalid."),
    };

    let original_host = parts.headers.get(header::HOST).cloned();
    let mut headers = parts.headers;
    remove_hop_by_hop(&mut headers);

    if (context.rewrite_host || !headers.contains_key(header::HOST))
        && let Ok(value) = HeaderValue::from_str(&context.authority())
    {
        headers.insert(header::HOST, value);
    }

    if let Some(host) = original_host {
        headers
            .entry(HeaderName::from_static("x-forwarded-host"))
            .or_insert(host);
    }
    if let Ok(value) = HeaderValue::from_str(context.scheme.as_str()) {
        headers.insert(HeaderName::from_static("x-forwarded-proto"), value);
    }

    let mut upstream = Request::builder()
        .method(parts.method)
        .uri(uri)
        .version(Version::HTTP_11)
        .body(body)
        .unwrap_or_else(|_| Request::new(Body::empty()));
    *upstream.headers_mut() = headers;

    let response = match context.client.request(upstream).await {
        Ok(response) => response,
        Err(error) => {
            return bad_gateway(&format!("The local server could not be reached: {error}"));
        }
    };

    let (mut parts, body) = response.into_parts();
    remove_hop_by_hop(&mut parts.headers);
    if let Some(length) = parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        context.stats.add_bytes_out(length);
    }

    let body = Body::from_stream(body.into_data_stream());
    Response::from_parts(parts, body)
}

async fn handle_websocket(context: ProxyContext, mut parts: Parts, _body: Body) -> Response {
    let protocol = parts.headers.get(header::SEC_WEBSOCKET_PROTOCOL).cloned();
    let path = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &context).await {
        Ok(upgrade) => upgrade,
        Err(error) => return error.into_response(),
    };

    let scheme = match context.scheme {
        ProxyScheme::Http => "ws",
        ProxyScheme::Https => "wss",
    };
    let url = format!("{scheme}://{}{path}", context.authority());
    context.stats.request(0);
    let ws_tls = context.ws_tls.clone();

    upgrade
        .on_upgrade(move |socket| async move {
            if let Err(error) = bridge_websocket(socket, url, protocol, ws_tls).await {
                eprintln!("OnionHoster WebSocket proxy error: {error}");
            }
        })
        .into_response()
}

async fn bridge_websocket(
    client: WebSocket,
    url: String,
    protocol: Option<HeaderValue>,
    ws_tls: Arc<rustls::ClientConfig>,
) -> Result<(), String> {
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(&url)
        .body(())
        .map_err(|error| format!("Could not build the upstream WebSocket request: {error}"))?;

    if let Some(protocol) = protocol {
        request
            .headers_mut()
            .insert(header::SEC_WEBSOCKET_PROTOCOL, protocol);
    }

    let (upstream, _) = tokio::time::timeout(
        Duration::from_secs(20),
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            None,
            false,
            Some(WebSocketConnector::Rustls(ws_tls)),
        ),
    )
    .await
    .map_err(|_| "The upstream WebSocket connection timed out.".to_string())?
    .map_err(|error| format!("Could not connect to the upstream WebSocket: {error}"))?;

    let (mut client_tx, mut client_rx) = client.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    loop {
        tokio::select! {
            message = client_rx.next() => {
                match message {
                    Some(Ok(message)) => {
                        if upstream_tx.send(to_upstream(message)).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            message = upstream_rx.next() => {
                match message {
                    Some(Ok(message)) => {
                        if client_tx.send(to_client(message)).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }
    }

    let _ = client_tx.close().await;
    let _ = upstream_tx.close().await;
    Ok(())
}

fn to_upstream(message: ClientMessage) -> UpstreamMessage {
    match message {
        ClientMessage::Text(text) => UpstreamMessage::Text(text.as_str().into()),
        ClientMessage::Binary(data) => UpstreamMessage::Binary(data),
        ClientMessage::Ping(data) => UpstreamMessage::Ping(data),
        ClientMessage::Pong(data) => UpstreamMessage::Pong(data),
        ClientMessage::Close(_) => UpstreamMessage::Close(None),
    }
}

fn to_client(message: UpstreamMessage) -> ClientMessage {
    match message {
        UpstreamMessage::Text(text) => ClientMessage::Text(text.as_str().into()),
        UpstreamMessage::Binary(data) => ClientMessage::Binary(data),
        UpstreamMessage::Ping(data) => ClientMessage::Ping(data),
        UpstreamMessage::Pong(data) => ClientMessage::Pong(data),
        UpstreamMessage::Close(_) => ClientMessage::Close(None),
        UpstreamMessage::Frame(_) => ClientMessage::Pong(Default::default()),
    }
}

fn is_websocket(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

fn remove_hop_by_hop(headers: &mut HeaderMap) {
    for name in [
        header::CONNECTION,
        HeaderName::from_static("keep-alive"),
        header::PROXY_AUTHENTICATE,
        header::PROXY_AUTHORIZATION,
        header::TE,
        header::TRAILER,
        header::TRANSFER_ENCODING,
        header::UPGRADE,
    ] {
        headers.remove(name);
    }
}

fn bad_gateway(message: &str) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        message.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_upstream_authority() {
        let context = ProxyContext {
            scheme: ProxyScheme::Http,
            host: "127.0.0.1".to_string(),
            port: 8080,
            rewrite_host: false,
            client: Client::builder(TokioExecutor::new()).build(
                HttpsConnectorBuilder::new()
                    .with_tls_config(tls_config(false))
                    .https_or_http()
                    .enable_http1()
                    .build(),
            ),
            ws_tls: Arc::new(tls_config(false)),
            stats: Arc::new(ServiceStats::default()),
        };
        assert_eq!(context.authority(), "127.0.0.1:8080");

        let default_http = ProxyContext {
            port: 80,
            ..context.clone()
        };
        assert_eq!(default_http.authority(), "127.0.0.1");

        let ipv6 = ProxyContext {
            host: "::1".to_string(),
            port: 443,
            scheme: ProxyScheme::Https,
            ..context
        };
        assert_eq!(ipv6.authority(), "[::1]");
    }
}
