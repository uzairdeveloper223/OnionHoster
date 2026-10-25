use crate::{community, stats::ServiceStats};
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Multipart, Path as AxumPath, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Method, StatusCode, header},
    response::Response,
    routing::{delete, get, post},
};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast;
use uuid::Uuid;

const HISTORY_LIMIT: usize = 100;
const QUERY_LIMIT_MAX: u32 = 100;

#[derive(Clone)]
struct ChatContext {
    db: Arc<Mutex<Connection>>,
    files_dir: PathBuf,
    stats: Arc<ServiceStats>,
    events: broadcast::Sender<String>,
    online: Arc<AtomicUsize>,
    rate: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentView {
    id: String,
    file_name: String,
    mime: String,
    size: u64,
    kind: String,
    url: String,
    download_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    id: String,
    client_id: String,
    author: String,
    body: String,
    created_at: u64,
    attachment: Option<AttachmentView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityResponse {
    client_id: String,
    tag: String,
    label: String,
}

#[derive(Serialize)]
struct HistoryResponse {
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<u64>,
    before_id: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct ClientQuery {
    client: Option<String>,
    download: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostMessage {
    client_id: String,
    name: Option<String>,
    body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OkResponse {
    ok: bool,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ServerEvent {
    History {
        messages: Vec<ChatMessage>,
        online: usize,
    },
    Message {
        message: Box<ChatMessage>,
    },
    Deleted {
        id: String,
    },
    Presence {
        online: usize,
    },
}

pub async fn start(
    data_dir: PathBuf,
    stats: Arc<ServiceStats>,
) -> Result<community::RunningServer, String> {
    community::spawn_router(build_router(context(data_dir, stats)?)).await
}

fn context(data_dir: PathBuf, stats: Arc<ServiceStats>) -> Result<ChatContext, String> {
    let (files_dir, database) = community::data_paths(data_dir)?;
    let connection = community::open_database(&database)?;
    initialize(&connection)?;

    let (events, _) = broadcast::channel(256);
    Ok(ChatContext {
        db: Arc::new(Mutex::new(connection)),
        files_dir,
        stats,
        events,
        online: Arc::new(AtomicUsize::new(0)),
        rate: Arc::new(Mutex::new(HashMap::new())),
    })
}

fn build_router(context: ChatContext) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/logo.svg", get(logo))
        .route("/favicon.svg", get(logo))
        .route("/favicon.ico", get(favicon_ico))
        .route("/api/chat/identity", get(identity))
        .route("/api/chat/messages", get(history).post(post_message))
        .route("/api/chat/messages/{id}", delete(delete_message))
        .route("/api/chat/files", post(post_file))
        .route("/api/chat/files/{id}", get(serve_attachment))
        .route("/ws", get(websocket))
        .layer(DefaultBodyLimit::max(
            (community::CHAT_MAX_FILE_BYTES + 1024 * 1024) as usize,
        ))
        .with_state(context)
}

fn initialize(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                client_id TEXT NOT NULL,
                display_name TEXT,
                body TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS attachments (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                mime TEXT NOT NULL,
                size INTEGER NOT NULL,
                storage_name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_chat_messages_created
                ON messages(created_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_chat_attachments_message
                ON attachments(message_id);",
        )
        .map_err(|error| format!("Could not initialize the chat database: {error}"))
}

async fn index(State(context): State<ChatContext>, method: Method) -> Response {
    community::page_response(
        &context.stats,
        &method,
        StatusCode::OK,
        include_str!("../assets/chat.html"),
        community::CHAT_CSP,
    )
}

async fn logo(State(context): State<ChatContext>, method: Method) -> Response {
    community::logo_response(&context.stats, &method)
}

async fn favicon_ico() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn identity(
    State(context): State<ChatContext>,
    Query(query): Query<ClientQuery>,
) -> Response {
    context.stats.request(0);
    let Some(client_id) = query.client else {
        return community::error_response(StatusCode::BAD_REQUEST, "A client ID is required.");
    };
    if !community::client_id_valid(&client_id) {
        return community::error_response(StatusCode::BAD_REQUEST, "The client ID is not valid.");
    }

    let label = community::identity_for(&client_id);
    let tag = label.trim_start_matches("anonymous#").to_string();
    community::json_response(
        &context.stats,
        StatusCode::OK,
        &IdentityResponse {
            client_id,
            tag,
            label,
        },
    )
}

async fn history(
    State(context): State<ChatContext>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    context.stats.request(0);
    let before = query.before.unwrap_or(0);
    let before_id = query.before_id.unwrap_or_default();
    let limit = query.limit.unwrap_or(50).clamp(1, QUERY_LIMIT_MAX) as i64;

    match load_messages(&context, before, &before_id, limit) {
        Ok(messages) => community::json_response(
            &context.stats,
            StatusCode::OK,
            &HistoryResponse { messages },
        ),
        Err(error) => community::error_response(StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
}

async fn post_message(
    State(context): State<ChatContext>,
    Json(input): Json<PostMessage>,
) -> Response {
    if !community::client_id_valid(&input.client_id) {
        return community::error_response(StatusCode::BAD_REQUEST, "The client ID is not valid.");
    }
    let body = match community::clean_body(&input.body) {
        Ok(body) => body,
        Err(error) => return community::error_response(StatusCode::BAD_REQUEST, &error),
    };
    if body.is_empty() {
        return community::error_response(StatusCode::BAD_REQUEST, "Write a message first.");
    }
    if !allow_message(&context, &input.client_id, 20) {
        return community::error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "You are sending messages too quickly.",
        );
    }

    let display_name = community::clean_name(input.name.as_deref());
    let message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        client_id: input.client_id.clone(),
        author: community::author_for(&input.client_id, display_name.as_deref()),
        body,
        created_at: crate::services::now_ms(),
        attachment: None,
    };

    if let Err(error) = insert_text_message(&context, &message, display_name.as_deref()) {
        return community::error_response(StatusCode::INTERNAL_SERVER_ERROR, &error);
    }

    context.stats.request(0);
    broadcast(
        &context,
        &ServerEvent::Message {
            message: Box::new(message.clone()),
        },
    );
    community::json_response(&context.stats, StatusCode::OK, &message)
}

async fn post_file(State(context): State<ChatContext>, mut multipart: Multipart) -> Response {
    let mut client_id = None;
    let mut name = None;
    let mut body = String::new();
    let mut uploaded = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => {
                return community::error_response(
                    StatusCode::BAD_REQUEST,
                    "The upload could not be read.",
                );
            }
        };

        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "clientId" => {
                client_id = Some(field.text().await.unwrap_or_default());
            }
            "name" => {
                name = Some(field.text().await.unwrap_or_default());
            }
            "body" => {
                body = field.text().await.unwrap_or_default();
            }
            "file" => {
                if uploaded.is_some() {
                    return community::error_response(
                        StatusCode::BAD_REQUEST,
                        "Only one file can be attached to a message.",
                    );
                }
                let original = field.file_name().unwrap_or("file").to_string();
                let bytes = match field.bytes().await {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        return community::error_response(
                            StatusCode::BAD_REQUEST,
                            "The file could not be read.",
                        );
                    }
                };
                if bytes.len() as u64 > community::CHAT_MAX_FILE_BYTES {
                    return community::error_response(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Chat files can be at most 25 MB.",
                    );
                }
                uploaded = Some((community::safe_file_name(&original), bytes));
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let Some(client_id) = client_id else {
        return community::error_response(StatusCode::BAD_REQUEST, "A client ID is required.");
    };
    if !community::client_id_valid(&client_id) {
        return community::error_response(StatusCode::BAD_REQUEST, "The client ID is not valid.");
    }
    let Some((file_name, bytes)) = uploaded else {
        return community::error_response(StatusCode::BAD_REQUEST, "Choose a file to attach.");
    };
    let body = match community::clean_body(&body) {
        Ok(body) => body,
        Err(error) => return community::error_response(StatusCode::BAD_REQUEST, &error),
    };
    if !allow_message(&context, &client_id, 10) {
        return community::error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "You are uploading too quickly.",
        );
    }

    let size = bytes.len() as u64;
    let attachment_id = Uuid::new_v4().to_string();
    let storage_name = format!("{attachment_id}.bin");
    let storage_path = context.files_dir.join(&storage_name);
    if let Err(error) = tokio::fs::write(&storage_path, &bytes).await {
        return community::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Could not save the file: {error}"),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&storage_path, std::fs::Permissions::from_mode(0o600));
    }

    let mime = community::mime_for_name(&file_name);
    let display_name = community::clean_name(name.as_deref());
    let message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        client_id: client_id.clone(),
        author: community::author_for(&client_id, display_name.as_deref()),
        body,
        created_at: crate::services::now_ms(),
        attachment: Some(AttachmentView {
            id: attachment_id.clone(),
            file_name: file_name.clone(),
            mime: mime.clone(),
            size,
            kind: community::media_kind(&mime).to_string(),
            url: format!("/api/chat/files/{attachment_id}"),
            download_url: format!("/api/chat/files/{attachment_id}?download=1"),
        }),
    };

    if let Err(error) = insert_file_message(
        &context,
        &message,
        display_name.as_deref(),
        &attachment_id,
        &file_name,
        &mime,
        size,
        &storage_name,
    ) {
        let _ = std::fs::remove_file(&storage_path);
        return community::error_response(StatusCode::INTERNAL_SERVER_ERROR, &error);
    }

    context.stats.request(size);
    broadcast(
        &context,
        &ServerEvent::Message {
            message: Box::new(message.clone()),
        },
    );
    community::json_response(&context.stats, StatusCode::OK, &message)
}

async fn delete_message(
    State(context): State<ChatContext>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ClientQuery>,
) -> Response {
    let Some(client_id) = query.client else {
        return community::error_response(StatusCode::BAD_REQUEST, "A client ID is required.");
    };
    if !community::client_id_valid(&client_id) {
        return community::error_response(StatusCode::BAD_REQUEST, "The client ID is not valid.");
    }

    let row = {
        let database = match context.db.lock() {
            Ok(database) => database,
            Err(_) => {
                return community::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The chat database is unavailable.",
                );
            }
        };
        database
            .query_row(
                "SELECT m.client_id, a.storage_name
                 FROM messages m
                 LEFT JOIN attachments a ON a.message_id = m.id
                 WHERE m.id = ?1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
    };

    let row = match row {
        Ok(row) => row,
        Err(error) => {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not read the message: {error}"),
            );
        }
    };
    let Some((owner, storage_name)) = row else {
        return community::error_response(StatusCode::NOT_FOUND, "The message was not found.");
    };
    if owner != client_id {
        return community::error_response(
            StatusCode::FORBIDDEN,
            "Only the author can delete this message.",
        );
    }

    {
        let database = match context.db.lock() {
            Ok(database) => database,
            Err(_) => {
                return community::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The chat database is unavailable.",
                );
            }
        };
        if let Err(error) = database.execute("DELETE FROM messages WHERE id = ?1", params![id]) {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not delete the message: {error}"),
            );
        }
    }

    if let Some(storage_name) = storage_name {
        let _ = std::fs::remove_file(context.files_dir.join(storage_name));
    }

    context.stats.request(0);
    broadcast(&context, &ServerEvent::Deleted { id });
    community::json_response(&context.stats, StatusCode::OK, &OkResponse { ok: true })
}

async fn serve_attachment(
    State(context): State<ChatContext>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ClientQuery>,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let row = {
        let database = match context.db.lock() {
            Ok(database) => database,
            Err(_) => {
                return community::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The chat database is unavailable.",
                );
            }
        };
        database
            .query_row(
                "SELECT file_name, mime, size, storage_name FROM attachments WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? as u64,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
    };

    let row = match row {
        Ok(row) => row,
        Err(error) => {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not read the attachment: {error}"),
            );
        }
    };
    let Some((file_name, mime, size, storage_name)) = row else {
        return community::error_response(StatusCode::NOT_FOUND, "The file was not found.");
    };

    let download = matches!(query.download.as_deref(), Some("1") | Some("true"));
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    community::file_response(
        &context.stats,
        &context.files_dir.join(storage_name),
        &file_name,
        &mime,
        size,
        range,
        download,
        &method,
    )
    .await
}

async fn websocket(ws: WebSocketUpgrade, State(context): State<ChatContext>) -> Response {
    ws.on_upgrade(move |socket| socket_loop(socket, context))
}

#[allow(clippy::collapsible_match)]
async fn socket_loop(socket: WebSocket, context: ChatContext) {
    let online = context.online.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = context
        .events
        .send(event_json(&ServerEvent::Presence { online }));
    let mut receiver = context.events.subscribe();
    let initial = ServerEvent::History {
        messages: load_messages(&context, 0, "", HISTORY_LIMIT as i64).unwrap_or_default(),
        online,
    };
    let (mut sender, mut incoming) = socket.split();

    if let Ok(payload) = serde_json::to_string(&initial)
        && sender.send(Message::Text(payload.into())).await.is_err()
    {
        finish_socket(&context);
        return;
    }

    loop {
        tokio::select! {
            message = incoming.next() => {
                match message {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            event = receiver.recv() => {
                match event {
                    Ok(payload) => {
                        if sender.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let current_online = context.online.load(Ordering::Relaxed);
                        let refresh = ServerEvent::History {
                            messages: load_messages(&context, 0, "", HISTORY_LIMIT as i64)
                                .unwrap_or_default(),
                            online: current_online,
                        };
                        if let Ok(payload) = serde_json::to_string(&refresh)
                            && sender.send(Message::Text(payload.into())).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    finish_socket(&context);
}

fn finish_socket(context: &ChatContext) {
    let online = context
        .online
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_sub(1))
        })
        .map(|value| value.saturating_sub(1))
        .unwrap_or(0);
    let _ = context
        .events
        .send(event_json(&ServerEvent::Presence { online }));
}

fn broadcast(context: &ChatContext, event: &ServerEvent) {
    let _ = context.events.send(event_json(event));
}

fn event_json(event: &ServerEvent) -> String {
    serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string())
}

fn allow_message(context: &ChatContext, client_id: &str, limit: usize) -> bool {
    let Ok(mut rates) = context.rate.lock() else {
        return false;
    };
    let now = Instant::now();
    let window = Duration::from_secs(30);
    if rates.len() > 5000 {
        rates.retain(|_, timestamps| {
            timestamps.retain(|timestamp| now.duration_since(*timestamp) < window);
            !timestamps.is_empty()
        });
    }
    let timestamps = rates.entry(client_id.to_string()).or_default();
    timestamps.retain(|timestamp| now.duration_since(*timestamp) < window);
    if timestamps.len() >= limit {
        return false;
    }
    timestamps.push(now);
    true
}

fn load_messages(
    context: &ChatContext,
    before: u64,
    before_id: &str,
    limit: i64,
) -> Result<Vec<ChatMessage>, String> {
    let database = context
        .db
        .lock()
        .map_err(|_| "The chat database is unavailable.".to_string())?;
    let mut statement = database
        .prepare(
            "SELECT m.id, m.client_id, m.display_name, m.body, m.created_at,
                    a.id, a.file_name, a.mime, a.size
             FROM messages m
             LEFT JOIN attachments a ON a.message_id = m.id
             WHERE (?1 = 0 OR m.created_at < ?1 OR (m.created_at = ?1 AND m.id < ?2))
             ORDER BY m.created_at DESC, m.id DESC
             LIMIT ?3",
        )
        .map_err(|error| format!("Could not prepare the chat history: {error}"))?;

    let rows = statement
        .query_map(params![before as i64, before_id, limit], message_from_row)
        .map_err(|error| format!("Could not read the chat history: {error}"))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|error| format!("Could not read a chat message: {error}"))?);
    }
    messages.reverse();
    Ok(messages)
}

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatMessage> {
    let client_id: String = row.get(1)?;
    let display_name: Option<String> = row.get(2)?;
    let attachment_id: Option<String> = row.get(5)?;
    let attachment = if let Some(id) = attachment_id {
        let mime: String = row.get(7)?;
        Some(AttachmentView {
            id: id.clone(),
            file_name: row.get(6)?,
            mime: mime.clone(),
            size: row.get::<_, i64>(8)? as u64,
            kind: community::media_kind(&mime).to_string(),
            url: format!("/api/chat/files/{id}"),
            download_url: format!("/api/chat/files/{id}?download=1"),
        })
    } else {
        None
    };

    Ok(ChatMessage {
        id: row.get(0)?,
        author: community::author_for(&client_id, display_name.as_deref()),
        client_id,
        body: row.get(3)?,
        created_at: row.get::<_, i64>(4)? as u64,
        attachment,
    })
}

fn insert_text_message(
    context: &ChatContext,
    message: &ChatMessage,
    display_name: Option<&str>,
) -> Result<(), String> {
    let database = context
        .db
        .lock()
        .map_err(|_| "The chat database is unavailable.".to_string())?;
    database
        .execute(
            "INSERT INTO messages (id, client_id, display_name, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message.id,
                message.client_id,
                display_name,
                message.body,
                message.created_at as i64
            ],
        )
        .map_err(|error| format!("Could not save the message: {error}"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_file_message(
    context: &ChatContext,
    message: &ChatMessage,
    display_name: Option<&str>,
    attachment_id: &str,
    file_name: &str,
    mime: &str,
    size: u64,
    storage_name: &str,
) -> Result<(), String> {
    let mut database = context
        .db
        .lock()
        .map_err(|_| "The chat database is unavailable.".to_string())?;
    let transaction = database
        .transaction()
        .map_err(|error| format!("Could not start the chat transaction: {error}"))?;
    let used: i64 = transaction
        .query_row(
            "SELECT COALESCE(SUM(size), 0) FROM attachments",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not check chat storage usage: {error}"))?;
    if used as u64 + size > community::CHAT_QUOTA_BYTES {
        return Err("The public chat storage quota of 1 GB has been reached.".to_string());
    }

    transaction
        .execute(
            "INSERT INTO messages (id, client_id, display_name, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message.id,
                message.client_id,
                display_name,
                message.body,
                message.created_at as i64
            ],
        )
        .map_err(|error| format!("Could not save the message: {error}"))?;
    transaction
        .execute(
            "INSERT INTO attachments
                (id, message_id, file_name, mime, size, storage_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attachment_id,
                message.id,
                file_name,
                mime,
                size as i64,
                storage_name,
                message.created_at as i64
            ],
        )
        .map_err(|error| format!("Could not save the attachment: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Could not finish the chat transaction: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_context() -> (ChatContext, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "onionhoster-chat-test-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let context =
            context(root.clone(), Arc::new(ServiceStats::default())).expect("create context");
        (context, root)
    }

    #[tokio::test]
    async fn serves_the_app_logo_and_favicon() {
        let (context, root) = test_context();
        let app = build_router(context);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/logo.svg")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("logo");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content type"),
            "image/svg+xml; charset=utf-8"
        );
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert!(String::from_utf8_lossy(&bytes).contains("OnionHoster"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/favicon.ico")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("favicon");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("index");
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let html = String::from_utf8_lossy(&bytes);
        assert!(html.contains("src=\"/logo.svg\""));
        assert!(html.contains("rel=\"icon\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn posts_and_reads_json_messages() {
        let (context, root) = test_context();
        let app = build_router(context);
        let payload = serde_json::json!({
            "clientId": "test-client-1234",
            "name": "Ada",
            "body": "hello"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/chat/messages")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .expect("request"),
            )
            .await
            .expect("post message");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let message: ChatMessage = serde_json::from_slice(&bytes).expect("message json");
        assert_eq!(message.author, "Ada");
        assert_eq!(message.body, "hello");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/chat/messages")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("history");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).expect("history json");
        assert_eq!(payload["messages"][0]["body"], "hello");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn uploads_serves_and_deletes_a_chat_file() {
        let (context, root) = test_context();
        let app = build_router(context);
        let boundary = "onionhoster-test-boundary";
        let multipart = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"clientId\"\r\n\r\n\
             test-client-1234\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"name\"\r\n\r\n\
             Ada\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"body\"\r\n\r\n\
             look\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             hello world\r\n\
             --{boundary}--\r\n"
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/chat/files")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(multipart))
                    .expect("request"),
            )
            .await
            .expect("upload");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let message: ChatMessage = serde_json::from_slice(&bytes).expect("message json");
        let attachment = message.attachment.expect("attachment");
        assert_eq!(attachment.file_name, "hello.txt");
        assert_eq!(attachment.kind, "file");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&attachment.url)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("download");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert_eq!(&bytes[..], b"hello world");

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!(
                        "/api/chat/messages/{}?client=test-client-1234",
                        message.id
                    ))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("delete");
        assert_eq!(response.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn message_rows_build_attachment_urls() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        initialize(&connection).expect("initialize database");
        connection
            .execute(
                "INSERT INTO messages VALUES ('m1', 'client-1234', 'Zed', 'hello', 10)",
                [],
            )
            .expect("insert message");
        connection
            .execute(
                "INSERT INTO attachments VALUES
                    ('a1', 'm1', 'photo.png', 'image/png', 12, 'a1.bin', 10)",
                [],
            )
            .expect("insert attachment");

        let mut statement = connection
            .prepare(
                "SELECT m.id, m.client_id, m.display_name, m.body, m.created_at,
                        a.id, a.file_name, a.mime, a.size
                 FROM messages m LEFT JOIN attachments a ON a.message_id = m.id",
            )
            .expect("prepare");
        let message = statement
            .query_row([], message_from_row)
            .expect("read message");
        assert_eq!(message.author, "Zed");
        let attachment = message.attachment.expect("attachment");
        assert_eq!(attachment.kind, "image");
        assert_eq!(attachment.url, "/api/chat/files/a1");
    }
}
