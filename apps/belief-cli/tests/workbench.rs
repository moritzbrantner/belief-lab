use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct Workbench {
    child: Child,
    authority: String,
    token: String,
}
impl Workbench {
    fn start() -> Self {
        Self::start_with(&[])
    }
    fn start_with(options: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_belief"))
            .arg("workbench")
            .args(options)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = sender.send(result); // The receiver can time out and close during cleanup.
        });
        let line = match receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(line)) => line,
            result => {
                child.kill().unwrap();
                child.wait().unwrap();
                reader.join().unwrap();
                panic!("startup failed: {result:?}");
            }
        };
        reader.join().unwrap();
        let (base, token) = line.trim().split_once("/#token=").unwrap();
        Self {
            child,
            authority: base.strip_prefix("http://").unwrap().into(),
            token: token.into(),
        }
    }
    fn request(
        &self,
        method: &str,
        path: &str,
        host: &str,
        headers: &str,
        body: &str,
    ) -> (u16, String) {
        let mut socket = TcpStream::connect(&self.authority).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(socket,"{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n{body}",body.len()).unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        (
            head.split_whitespace().nth(1).unwrap().parse().unwrap(),
            body.into(),
        )
    }
    fn headers(&self) -> String {
        format!(
            "Origin: http://{}\r\nX-Belief-Token: {}\r\nContent-Type: application/json\r\n",
            self.authority, self.token
        )
    }
}
impl Drop for Workbench {
    fn drop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}
#[test]
fn native_workbench_serves_embedded_assets_without_a_site_build() {
    let server = Workbench::start();
    assert!(server.authority.starts_with("127.0.0.1:"));
    assert_eq!(server.token.len(), 64);
    for path in [
        "/",
        "/local.js",
        "/style.css",
        "/local-translations.js",
        "/example.json",
    ] {
        let (status, body) = server.request("GET", path, &server.authority, "", "");
        assert_eq!(status, 200, "{path}");
        assert!(!body.contains(&server.token));
    }
    assert_eq!(
        server
            .request("GET", "/Cargo.toml", &server.authority, "", "")
            .0,
        404
    );
    assert_eq!(
        server.request("GET", "/", "attacker.example", "", "").0,
        403
    );
}
#[test]
fn session_origin_content_type_and_size_are_enforced_before_execution() {
    let server = Workbench::start();
    let input = include_str!("../../../examples/decisions/fixture.json");
    for headers in [
        String::new(),
        server.headers().replace(&server.token, "wrong"),
        server.headers().replace(
            &format!("Origin: http://{}", server.authority),
            "Origin: https://attacker.example",
        ),
    ] {
        assert_eq!(
            server
                .request("POST", "/api/decide", &server.authority, &headers, input)
                .0,
            403
        );
    }
    assert_eq!(
        server
            .request("OPTIONS", "/api/decide", &server.authority, "", "")
            .0,
        405
    );
    assert_eq!(
        server
            .request(
                "POST",
                "/api/decide",
                &server.authority,
                &server.headers().replace("application/json", "text/plain"),
                input
            )
            .0,
        415
    );
    let large = "x".repeat(belief_cli::decision::MAX_INPUT_BYTES + 1);
    assert_eq!(
        server
            .request(
                "POST",
                "/api/decide",
                &server.authority,
                &server.headers(),
                &large
            )
            .0,
        413
    );
    let (status, body) = server.request(
        "POST",
        "/api/decide",
        &server.authority,
        &server.headers(),
        input,
    );
    assert_eq!(status, 200);
    let result: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(result["belief"]["value"], 0.95);
}
#[test]
fn policy_and_malformed_input_errors_remain_structured_and_recoverable() {
    let server = Workbench::start();
    let mut input: Value =
        serde_json::from_str(include_str!("../../../examples/decisions/fixture.json")).unwrap();
    input["policy"]["BELIEF_POLICY_PROFILE"] = "observe_only".into();
    for (body, code) in [
        (input.to_string(), "policy_denied"),
        ("{".into(), "invalid_request"),
    ] {
        let (status, result) = server.request(
            "POST",
            "/api/decide",
            &server.authority,
            &server.headers(),
            &body,
        );
        assert_eq!(status, 422);
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap()["error"]["code"],
            code
        );
    }
}

#[test]
fn concurrent_submissions_are_refused_until_the_current_upload_finishes() {
    let server = Workbench::start();
    let input = include_str!("../../../examples/decisions/fixture.json");
    let mut socket = TcpStream::connect(&server.authority).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(socket, "POST /api/decide HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nExpect: 100-continue\r\nContent-Length: {}\r\n{}\r\n", server.authority, input.len(), server.headers()).unwrap();
    // Receiving 100 Continue proves the first handler is waiting on its body with
    // the execution slot held; no timing guesses or model downloads are needed.
    let mut interim = [0; 25];
    socket.read_exact(&mut interim).unwrap();
    assert_eq!(&interim, b"HTTP/1.1 100 Continue\r\n\r\n");
    let (status, body) = server.request(
        "POST",
        "/api/decide",
        &server.authority,
        &server.headers(),
        input,
    );
    assert_eq!(status, 409);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["error"]["code"],
        "busy"
    );
    socket.write_all(input.as_bytes()).unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(
        server
            .request(
                "POST",
                "/api/decide",
                &server.authority,
                &server.headers(),
                input
            )
            .0,
        200
    );
}

fn project(id: &str) -> Value {
    serde_json::json!({
        "schema": "belief_investigation_project", "schemaVersion": 1,
        "investigation": {"id": id, "name": "Research A", "createdAt": "2026-10-22T08:00:00Z"},
        "evidenceBatches": [{
            "importedAt": "2026-10-22T08:01:00Z",
            "batch": serde_json::from_str::<Value>(include_str!("../../../examples/person-attributions/evidence.json")).unwrap()
        }],
        "annotations": serde_json::from_str::<Value>(include_str!("../../../examples/person-attributions/attributions.json")).unwrap()
    })
}

#[test]
fn saved_investigations_are_validated_session_guarded_files() {
    let data =
        std::env::temp_dir().join(format!("belief-lab-investigations-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let server = Workbench::start_with(&[
        "--data-dir",
        data.to_str().unwrap(),
        "--site",
        "missing-site",
    ]);
    let headers = server.headers();
    let (status, body) = server.request(
        "GET",
        "/api/investigations",
        &server.authority,
        &headers,
        "",
    );
    assert_eq!((status, body.as_str()), (200, "[]"));

    let valid = project("research-a").to_string();
    for bad_headers in [String::new(), headers.replace(&server.token, "wrong")] {
        let status = server
            .request(
                "PUT",
                "/api/investigations/research-a",
                &server.authority,
                &bad_headers,
                &valid,
            )
            .0;
        assert_eq!(status, 403);
    }
    let foreign = headers.replace(
        &format!("Origin: http://{}", server.authority),
        "Origin: https://attacker.example",
    );
    assert_eq!(
        server
            .request(
                "GET",
                "/api/investigations",
                &server.authority,
                &foreign,
                ""
            )
            .0,
        403
    );
    assert_eq!(
        server
            .request(
                "PUT",
                "/api/investigations/other-id",
                &server.authority,
                &headers,
                &valid
            )
            .0,
        400
    );
    assert_eq!(
        server
            .request(
                "GET",
                "/api/investigations/..%2Fsecret",
                &server.authority,
                &headers,
                ""
            )
            .0,
        400
    );
    let mut tampered = project("research-a");
    tampered["annotations"]["observations"][1]["stance"] = serde_json::json!("endorses");
    assert_eq!(
        server
            .request(
                "PUT",
                "/api/investigations/research-a",
                &server.authority,
                &headers,
                &tampered.to_string()
            )
            .0,
        422
    );
    assert!(
        !data.join("research-a.json").exists(),
        "a refused project reached the disk"
    );

    assert_eq!(
        server
            .request(
                "PUT",
                "/api/investigations/research-a",
                &server.authority,
                &headers,
                &valid
            )
            .0,
        204
    );
    let (status, body) = server.request(
        "GET",
        "/api/investigations/research-a",
        &server.authority,
        &headers,
        "",
    );
    assert_eq!(status, 200);
    assert_eq!(body, valid);
    let (_, listed) = server.request(
        "GET",
        "/api/investigations",
        &server.authority,
        &headers,
        "",
    );
    let listed: Value = serde_json::from_str(&listed).unwrap();
    assert_eq!(listed[0]["name"], "Research A");
    assert_eq!(
        server
            .request(
                "DELETE",
                "/api/investigations/research-a",
                &server.authority,
                &headers,
                ""
            )
            .0,
        204
    );
    assert!(!data.join("research-a.json").exists());

    // Without a site build the investigation pages explain how to build them.
    let (status, body) = server.request("GET", "/people.html", &server.authority, "", "");
    assert_eq!(status, 404);
    assert!(body.contains("build-pages.sh"));
    assert_eq!(
        server
            .request(
                "GET",
                "/pkg/..%2F..%2FCargo.toml",
                &server.authority,
                "",
                ""
            )
            .0,
        404
    );
    let _ = std::fs::remove_dir_all(&data);
}
