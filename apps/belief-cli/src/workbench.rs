//! A session-scoped loopback UI over the existing one-decision operation.
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use belief_cli::decision::{Failure, MAX_INPUT_BYTES};
use serde_json::Value;
use std::io::{self, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

type Execute = fn(&str) -> Result<Value, Failure>;
#[derive(Clone)]
struct Session {
    authority: String,
    token: String,
    busy: Arc<AtomicBool>,
    execute: Execute,
}
struct Running(Arc<AtomicBool>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub fn run(execute: Execute) -> Result<(), Box<dyn std::error::Error>> {
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
            .layer(axum::middleware::from_fn_with_state(
                session.clone(),
                guard_host,
            ))
            .with_state(session);
        println!("http://{authority}/#token={token}");
        io::stdout().flush()?;
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
