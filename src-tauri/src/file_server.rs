use crate::{community, stats::ServiceStats};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart, Path as AxumPath, Query, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::Response,
    routing::{delete, get},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

const LIST_LIMIT: i64 = 1000;

#[derive(Clone)]
struct FileContext {
    db: Arc<Mutex<Connection>>,
    files_dir: PathBuf,
    stats: Arc<ServiceStats>,
    rate: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileView {
    id: String,
    file_name: String,
    mime: String,
    size: u64,
    kind: &'static str,
    created_at: u64,
    url: String,
    download_url: String,
    owned: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResponse {
    files: Vec<FileView>,
    used: u64,
    quota: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OkResponse {
    ok: bool,
}

#[derive(Deserialize)]
struct ListQuery {
    client: Option<String>,
}

#[derive(Deserialize)]
struct ClientQuery {
    client: Option<String>,
    download: Option<String>,
}

pub async fn start(
    data_dir: PathBuf,
    stats: Arc<ServiceStats>,
) -> Result<community::RunningServer, String> {
    community::spawn_router(build_router(context(data_dir, stats)?)).await
}

fn context(data_dir: PathBuf, stats: Arc<ServiceStats>) -> Result<FileContext, String> {
    let (files_dir, database) = community::data_paths(data_dir)?;
    let connection = community::open_database(&database)?;
    initialize(&connection)?;

    Ok(FileContext {
        db: Arc::new(Mutex::new(connection)),
        files_dir,
        stats,
        rate: Arc::new(Mutex::new(HashMap::new())),
    })
}

fn build_router(context: FileContext) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/logo.svg", get(logo))
        .route("/favicon.svg", get(logo))
        .route("/favicon.ico", get(favicon_ico))
        .route("/api/files", get(list).post(upload))
        .route("/api/files/{id}", delete(remove))
        .route("/api/files/{id}/content", get(serve_file))
        .layer(DefaultBodyLimit::max(
            (community::FILE_MAX_BYTES + 1024 * 1024) as usize,
        ))
        .with_state(context)
}

fn initialize(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY,
                client_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                mime TEXT NOT NULL,
                size INTEGER NOT NULL,
                storage_name TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_files_created
                ON files(created_at DESC, id DESC);",
        )
        .map_err(|error| format!("Could not initialize the file-sharing database: {error}"))
}

async fn index(State(context): State<FileContext>, method: Method) -> Response {
    community::page_response(
        &context.stats,
        &method,
        StatusCode::OK,
        include_str!("../assets/files.html"),
        community::FILES_CSP,
    )
}

async fn logo(State(context): State<FileContext>, method: Method) -> Response {
    community::logo_response(&context.stats, &method)
}

async fn favicon_ico() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn list(State(context): State<FileContext>, Query(query): Query<ListQuery>) -> Response {
    context.stats.request(0);
    let client = query
        .client
        .filter(|value| community::client_id_valid(value));
    let database = match context.db.lock() {
        Ok(database) => database,
        Err(_) => {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The file-sharing database is unavailable.",
            );
        }
    };

    let mut statement = match database.prepare(
        "SELECT id, client_id, file_name, mime, size, created_at
         FROM files ORDER BY created_at DESC, id DESC LIMIT ?1",
    ) {
        Ok(statement) => statement,
        Err(error) => {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not read the file list: {error}"),
            );
        }
    };

    let rows = match statement.query_map(params![LIST_LIMIT], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)? as u64,
            row.get::<_, i64>(5)? as u64,
        ))
    }) {
        Ok(rows) => rows,
        Err(error) => {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not read the file list: {error}"),
            );
        }
    };

    let mut files = Vec::new();
    for row in rows {
        let (id, owner, file_name, mime, size, created_at) = match row {
            Ok(row) => row,
            Err(error) => {
                return community::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Could not read a file: {error}"),
                );
            }
        };
        let owned = client.as_deref() == Some(owner.as_str());
        files.push(file_view(&id, &file_name, &mime, size, created_at, owned));
    }

    let used = database
        .query_row("SELECT COALESCE(SUM(size), 0) FROM files", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0) as u64;

    community::json_response(
        &context.stats,
        StatusCode::OK,
        &ListResponse {
            files,
            used,
            quota: community::FILE_QUOTA_BYTES,
        },
    )
}

async fn upload(State(context): State<FileContext>, mut multipart: Multipart) -> Response {
    let mut client_id = None;
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
            "file" => {
                if uploaded.is_some() {
                    return community::error_response(
                        StatusCode::BAD_REQUEST,
                        "Upload one file at a time.",
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
                if bytes.len() as u64 > community::FILE_MAX_BYTES {
                    return community::error_response(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Public files can be at most 50 MB.",
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
        return community::error_response(StatusCode::BAD_REQUEST, "Choose a file to upload.");
    };
    if !allow_upload(&context, &client_id) {
        return community::error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "You are uploading too quickly.",
        );
    }

    let size = bytes.len() as u64;
    let id = Uuid::new_v4().to_string();
    let storage_name = format!("{id}.bin");
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
    let created_at = crate::services::now_ms();
    if let Err(error) = insert_file(
        &context,
        &id,
        &client_id,
        &file_name,
        &mime,
        size,
        &storage_name,
        created_at,
    ) {
        let _ = std::fs::remove_file(&storage_path);
        return community::error_response(StatusCode::INTERNAL_SERVER_ERROR, &error);
    }

    context.stats.request(size);
    community::json_response(
        &context.stats,
        StatusCode::OK,
        &file_view(&id, &file_name, &mime, size, created_at, true),
    )
}

async fn remove(
    State(context): State<FileContext>,
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
                    "The file-sharing database is unavailable.",
                );
            }
        };
        database
            .query_row(
                "SELECT client_id, storage_name FROM files WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
    };

    let row = match row {
        Ok(row) => row,
        Err(error) => {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not read the file: {error}"),
            );
        }
    };
    let Some((owner, storage_name)) = row else {
        return community::error_response(StatusCode::NOT_FOUND, "The file was not found.");
    };
    if owner != client_id {
        return community::error_response(
            StatusCode::FORBIDDEN,
            "Only the uploader can delete this file.",
        );
    }

    {
        let database = match context.db.lock() {
            Ok(database) => database,
            Err(_) => {
                return community::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "The file-sharing database is unavailable.",
                );
            }
        };
        if let Err(error) = database.execute("DELETE FROM files WHERE id = ?1", params![id]) {
            return community::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Could not delete the file: {error}"),
            );
        }
    }

    let _ = std::fs::remove_file(context.files_dir.join(storage_name));
    context.stats.request(0);
    community::json_response(&context.stats, StatusCode::OK, &OkResponse { ok: true })
}

async fn serve_file(
    State(context): State<FileContext>,
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
                    "The file-sharing database is unavailable.",
                );
            }
        };
        database
            .query_row(
                "SELECT file_name, mime, size, storage_name FROM files WHERE id = ?1",
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
                &format!("Could not read the file: {error}"),
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

fn file_view(
    id: &str,
    file_name: &str,
    mime: &str,
    size: u64,
    created_at: u64,
    owned: bool,
) -> FileView {
    FileView {
        id: id.to_string(),
        file_name: file_name.to_string(),
        mime: mime.to_string(),
        size,
        kind: community::media_kind(mime),
        created_at,
        url: format!("/api/files/{id}/content"),
        download_url: format!("/api/files/{id}/content?download=1"),
        owned,
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_file(
    context: &FileContext,
    id: &str,
    client_id: &str,
    file_name: &str,
    mime: &str,
    size: u64,
    storage_name: &str,
    created_at: u64,
) -> Result<(), String> {
    let mut database = context
        .db
        .lock()
        .map_err(|_| "The file-sharing database is unavailable.".to_string())?;
    let transaction = database
        .transaction()
        .map_err(|error| format!("Could not start the file transaction: {error}"))?;
    let used: i64 = transaction
        .query_row("SELECT COALESCE(SUM(size), 0) FROM files", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("Could not check file storage usage: {error}"))?;
    if used as u64 + size > community::FILE_QUOTA_BYTES {
        return Err("The public file storage quota of 2 GB has been reached.".to_string());
    }

    transaction
        .execute(
            "INSERT INTO files
                (id, client_id, file_name, mime, size, storage_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                client_id,
                file_name,
                mime,
                size as i64,
                storage_name,
                created_at as i64
            ],
        )
        .map_err(|error| format!("Could not save the file: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Could not finish the file transaction: {error}"))?;
    Ok(())
}

fn allow_upload(context: &FileContext, client_id: &str) -> bool {
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
    if timestamps.len() >= 10 {
        return false;
    }
    timestamps.push(now);
    true
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

    fn test_context() -> (FileContext, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "onionhoster-files-test-{}-{}",
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
    async fn uploads_lists_serves_and_deletes_a_file() {
        let (context, root) = test_context();
        let app = build_router(context);
        let boundary = "onionhoster-test-boundary";
        let multipart = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"clientId\"\r\n\r\n\
             test-client-1234\r\n\
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
                    .uri("/api/files")
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
        let file: serde_json::Value = serde_json::from_slice(&bytes).expect("file json");
        assert_eq!(file["fileName"], "hello.txt");
        assert_eq!(file["owned"], true);
        let id = file["id"].as_str().expect("file id").to_string();
        let url = file["url"].as_str().expect("file url").to_string();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/files?client=test-client-1234")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("list");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).expect("list json");
        assert_eq!(payload["files"][0]["fileName"], "hello.txt");
        assert_eq!(payload["used"], 11);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&url)
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
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&url)
                    .header("range", "bytes=0-4")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("range");
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert_eq!(&bytes[..], b"hello");

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/files/{id}?client=test-client-1234"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("delete");
        assert_eq!(response.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(root);
    }
}
