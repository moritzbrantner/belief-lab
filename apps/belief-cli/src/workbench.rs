//! A session-scoped loopback UI over the existing one-decision operation, plus the person-belief
//! investigation pages with their saved investigations stored as files on this computer.
use axum::{
    body::to_bytes,
    extract::{Path as UrlPath, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use belief_cli::decision::{Failure, MAX_INPUT_BYTES};
use serde_json::Value;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

type Execute = fn(&str) -> Result<Value, Failure>;
const MAX_PROJECT_BYTES: usize = 4 * MAX_INPUT_BYTES;

/// Where the built investigation pages and the saved investigations live.
pub struct Options {
    /// The output of `scripts/build-pages.sh`; the investigation pages need its WASM bundle.
    pub site: PathBuf,
    pub data_dir: PathBuf,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self {
            site: PathBuf::from("target/pages"),
            data_dir: default_data_dir().ok_or("no home directory; pass --data-dir")?,
        };
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            let value = args.next().ok_or(format!("{flag} needs a value"))?;
            match flag.as_str() {
                "--site" => options.site = PathBuf::from(value),
                "--data-dir" => options.data_dir = PathBuf::from(value),
                _ => return Err(format!("unknown workbench option {flag}")),
            }
        }
        Ok(options)
    }
}

/// The platform data directory: XDG on Linux, Application Support on macOS, APPDATA on Windows.
fn default_data_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
    };
    base.map(|base| base.join("belief-lab").join("investigations"))
}

#[derive(Clone)]
struct Session {
    authority: String,
    token: String,
    busy: Arc<AtomicBool>,
    execute: Execute,
    site: Arc<PathBuf>,
    data_dir: Arc<PathBuf>,
}
struct Running(Arc<AtomicBool>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub fn run(execute: Execute, options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let authority = listener.local_addr()?.to_string();
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(io::Error::other)?;
        let token = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let session = Session {
            authority: authority.clone(),
            token: token.clone(),
            busy: Arc::new(AtomicBool::new(false)),
            execute,
            site: Arc::new(options.site),
            data_dir: Arc::new(options.data_dir),
        };
        let app = Router::new()
            .route("/", get(index))
            .route(
                "/local.js",
                get(|| async {
                    asset("text/javascript", include_str!("../../../site/local.js"))
                }),
            )
            .route(
                "/local-translations.js",
                get(|| async {
                    asset("text/javascript", include_str!("../../../site/local-translations.js"))
                }),
            )
            .route(
                "/style.css",
                get(|| async {
                    asset("text/css", include_str!("../../../site/style.css"))
                }),
            )
            .route(
                "/example.json",
                get(|| async {
                    asset("application/json", include_str!("../../../examples/decisions/preference.json"))
                }),
            )
            .route(
                "/fixture.json",
                get(|| async {
                    asset("application/json", include_str!("../../../examples/decisions/fixture.json"))
                }),
            )
            .route("/api/decide", post(decide))
            .route("/people.html", get(site_page))
            .route("/people.js", get(site_script))
            .route("/pkg/{file}", get(site_package))
            .route("/examples/person-attributions/{file}", get(site_example))
            .route("/api/investigations", get(list_investigations))
            .route(
                "/api/investigations/{id}",
                get(read_investigation)
                    .put(write_investigation)
                    .delete(delete_investigation),
            )
            .layer(axum::middleware::from_fn_with_state(
                session.clone(),
                guard_host,
            ))
            .with_state(session.clone());
        println!("http://{authority}/#token={token}");
        io::stdout().flush()?;
        eprintln!(
            "Investigations: http://{authority}/people.html#token={token} (saved in {})",
            session.data_dir.display()
        );
        eprintln!("Local model workbench is ready. Open the URL above. Press Ctrl+C to stop. Model setup runs only when you submit a request.");
        // The session intentionally lasts until the user stops the CLI. All model work is
        // awaited; the established provider deadline bounds scoring after acquisition.
        axum::serve(listener, app).await
    })?;
    Ok(())
}

async fn index() -> Response {
    asset(
        "text/html; charset=utf-8",
        include_str!("../../../site/local.html"),
    )
}
fn asset(kind: &'static str, body: &'static str) -> Response {
    (
        [
            ("content-type", kind),
            ("cache-control", "no-store"),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
            ("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"),
        ],
        body,
    ).into_response()
}
fn failure(status: StatusCode, code: &'static str, message: impl std::fmt::Display) -> Response {
    (
        status,
        [("cache-control", "no-store")],
        Json(Failure::new(code, message).report()),
    )
        .into_response()
}
async fn guard_host(
    State(session): State<Session>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    if header(request.headers(), "host") != Some(session.authority.as_str()) {
        return failure(
            StatusCode::FORBIDDEN,
            "invalid_host",
            "use the exact loopback URL printed by the CLI",
        );
    }
    next.run(request).await
}
fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}
async fn decide(State(session): State<Session>, request: Request) -> Response {
    let origin = format!("http://{}", session.authority);
    if header(request.headers(), "origin") != Some(origin.as_str())
        || header(request.headers(), "x-belief-token") != Some(session.token.as_str())
    {
        return failure(
            StatusCode::FORBIDDEN,
            "invalid_session",
            "open the session URL printed by the CLI",
        );
    }
    if header(request.headers(), "content-type") != Some("application/json") {
        return failure(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_content_type",
            "expected application/json",
        );
    }
    if session
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return failure(
            StatusCode::CONFLICT,
            "busy",
            "another decision is still running",
        );
    }
    let running = Running(session.busy.clone());
    let bytes = match tokio::time::timeout(
        Duration::from_secs(15),
        to_bytes(request.into_body(), MAX_INPUT_BYTES),
    )
    .await
    {
        Err(_) => {
            return failure(
                StatusCode::REQUEST_TIMEOUT,
                "input_timeout",
                "request upload exceeded 15 seconds",
            )
        }
        Ok(Err(error)) => return failure(StatusCode::PAYLOAD_TOO_LARGE, "invalid_request", error),
        Ok(Ok(bytes)) => bytes,
    };
    let input = match String::from_utf8(bytes.to_vec()) {
        Ok(input) => input,
        Err(error) => return failure(StatusCode::BAD_REQUEST, "invalid_request", error),
    };
    let result = tokio::task::spawn_blocking(move || {
        // The slot remains held even if the browser disconnects; no second model can race
        // acquisition or replace the in-flight result under a different request.
        let _running = running;
        (session.execute)(&input)
    })
    .await;
    match result {
        Ok(Ok(value)) => ([("cache-control", "no-store")], Json(value)).into_response(),
        Ok(Err(error)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            [("cache-control", "no-store")],
            Json(error.report()),
        )
            .into_response(),
        Err(error) => failure(StatusCode::INTERNAL_SERVER_ERROR, "execution_failed", error),
    }
}

/// The session token from the printed URL is always required. Browsers omit `Origin` on
/// same-origin reads, so reads accept its absence; writes need the exact origin, and a foreign
/// origin is always refused.
fn session_failure(session: &Session, headers: &HeaderMap, write: bool) -> Option<Response> {
    let origin = format!("http://{}", session.authority);
    let origin_ok = match header(headers, "origin") {
        Some(value) => value == origin,
        None => !write && headers.get("origin").is_none(),
    };
    (!origin_ok || header(headers, "x-belief-token") != Some(session.token.as_str())).then(|| {
        failure(
            StatusCode::FORBIDDEN,
            "invalid_session",
            "open the session URL printed by the CLI",
        )
    })
}

fn site_asset(session: &Session, relative: &str, kind: &'static str) -> Response {
    match std::fs::read(session.site.join(relative)) {
        Ok(body) => (
            [
                ("content-type", kind),
                ("cache-control", "no-store"),
                ("referrer-policy", "no-referrer"),
                ("x-content-type-options", "nosniff"),
                // The investigation pages run the shared Rust attribution code as WebAssembly.
                ("content-security-policy", "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"),
            ],
            body,
        )
            .into_response(),
        Err(_) => failure(
            StatusCode::NOT_FOUND,
            "site_not_built",
            format!(
                "{} is missing; build the pages with scripts/build-pages.sh or pass --site",
                session.site.join(relative).display()
            ),
        ),
    }
}

/// Only plain file names from the built site; no separators or parent references.
fn plain_file(file: &str) -> bool {
    !file.is_empty()
        && !file.starts_with('.')
        && file
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn kind_of(file: &str) -> &'static str {
    match Path::new(file).extension().and_then(|value| value.to_str()) {
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("ts") => "text/plain",
        _ => "application/octet-stream",
    }
}

async fn site_page(State(session): State<Session>) -> Response {
    site_asset(&session, "people.html", "text/html; charset=utf-8")
}
async fn site_script(State(session): State<Session>) -> Response {
    site_asset(&session, "people.js", "text/javascript")
}
async fn site_package(State(session): State<Session>, UrlPath(file): UrlPath<String>) -> Response {
    if !plain_file(&file) {
        return failure(StatusCode::NOT_FOUND, "not_found", "unknown asset");
    }
    site_asset(&session, &format!("pkg/{file}"), kind_of(&file))
}
async fn site_example(State(session): State<Session>, UrlPath(file): UrlPath<String>) -> Response {
    if !plain_file(&file) {
        return failure(StatusCode::NOT_FOUND, "not_found", "unknown asset");
    }
    site_asset(
        &session,
        &format!("examples/person-attributions/{file}"),
        kind_of(&file),
    )
}

fn project_path(session: &Session, id: &str) -> Option<PathBuf> {
    belief_cli::project::valid_project_id(id).then(|| session.data_dir.join(format!("{id}.json")))
}

async fn list_investigations(State(session): State<Session>, headers: HeaderMap) -> Response {
    if let Some(denied) = session_failure(&session, &headers, false) {
        return denied;
    }
    let entries = match std::fs::read_dir(session.data_dir.as_ref()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ([("cache-control", "no-store")], Json(Vec::<Value>::new())).into_response()
        }
        Err(error) => return failure(StatusCode::INTERNAL_SERVER_ERROR, "storage_failed", error),
    };
    let mut listed = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(project) = serde_json::from_str::<belief_cli::project::ProjectDocument>(&text)
        else {
            continue;
        };
        let updated = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_millis().to_string());
        listed.push(serde_json::json!({
            "id": project.investigation.id,
            "name": project.investigation.name,
            "createdAt": project.investigation.created_at,
            // Milliseconds since the epoch, zero-padded so it sorts as text like the browser's ISO time.
            "updatedAt": updated.map(|value| format!("{value:0>16}")),
        }));
    }
    ([("cache-control", "no-store")], Json(listed)).into_response()
}

async fn read_investigation(
    State(session): State<Session>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Some(denied) = session_failure(&session, &headers, false) {
        return denied;
    }
    let Some(path) = project_path(&session, &id) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "invalid investigation id",
        );
    };
    match std::fs::read_to_string(path) {
        Ok(text) => (
            [
                ("content-type", "application/json"),
                ("cache-control", "no-store"),
            ],
            text,
        )
            .into_response(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => failure(
            StatusCode::NOT_FOUND,
            "not_found",
            "no saved investigation with this id",
        ),
        Err(error) => failure(StatusCode::INTERNAL_SERVER_ERROR, "storage_failed", error),
    }
}

async fn write_investigation(
    State(session): State<Session>,
    UrlPath(id): UrlPath<String>,
    request: Request,
) -> Response {
    if let Some(denied) = session_failure(&session, request.headers(), true) {
        return denied;
    }
    if header(request.headers(), "content-type") != Some("application/json") {
        return failure(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_content_type",
            "expected application/json",
        );
    }
    let Some(path) = project_path(&session, &id) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "invalid investigation id",
        );
    };
    let bytes = match to_bytes(request.into_body(), MAX_PROJECT_BYTES).await {
        Ok(bytes) => bytes,
        Err(error) => return failure(StatusCode::PAYLOAD_TOO_LARGE, "invalid_request", error),
    };
    let Ok(text) = String::from_utf8(bytes.to_vec()) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "project must be UTF-8",
        );
    };
    // Only projects the shared rules accept reach the disk, and only under their own id.
    let policy = std::collections::BTreeMap::from([(
        "BELIEF_POLICY_PROFILE".to_owned(),
        "semantic_research".to_owned(),
    )]);
    match belief_cli::project::open_project(&text, &policy) {
        Ok(report) if report.investigation.id == id => {}
        Ok(_) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_project",
                "project id does not match the storage path",
            )
        }
        Err(error) => return failure(StatusCode::UNPROCESSABLE_ENTITY, "invalid_project", error),
    }
    let write = || -> io::Result<()> {
        std::fs::create_dir_all(session.data_dir.as_ref())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, &text)?;
        std::fs::rename(&temporary, &path)
    };
    match write() {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => failure(StatusCode::INTERNAL_SERVER_ERROR, "storage_failed", error),
    }
}

async fn delete_investigation(
    State(session): State<Session>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Some(denied) = session_failure(&session, &headers, true) {
        return denied;
    }
    let Some(path) = project_path(&session, &id) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "invalid investigation id",
        );
    };
    match std::fs::remove_file(path) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => failure(
            StatusCode::NOT_FOUND,
            "not_found",
            "no saved investigation with this id",
        ),
        Err(error) => failure(StatusCode::INTERNAL_SERVER_ERROR, "storage_failed", error),
    }
}
