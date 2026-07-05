use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};
use tempfile::TempDir;
use tiny_http::{Header, Request, Response, Server};

/// Keeps a fake gateway alive until the expected request sequence completes.
struct FakeGateway {
    base_url: String,
    handle: JoinHandle<()>,
}

impl FakeGateway {
    /// Waits for the fake gateway thread so request assertions are surfaced.
    fn join(self) {
        self.handle.join().expect("fake gateway thread panicked");
    }
}

/// Starts a tiny HTTP server that validates a fixed request sequence.
fn start_gateway<F>(expected_requests: usize, mut handler: F) -> FakeGateway
where
    F: FnMut(usize, Request) + Send + 'static,
{
    let server = Server::http("127.0.0.1:0").expect("server should bind");
    let address = server
        .server_addr()
        .to_string()
        .replace("0.0.0.0", "127.0.0.1");
    let handle = thread::spawn(move || {
        for index in 0..expected_requests {
            let request = server
                .recv_timeout(Duration::from_secs(10))
                .expect("server receive should not fail")
                .expect("expected request did not arrive");
            handler(index, request);
        }
    });
    FakeGateway {
        base_url: format!("http://{address}"),
        handle,
    }
}

/// Runs the diskd binary with isolated config and auth environment.
fn run_diskd(home: &TempDir, base_url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diskd"))
        .env("DISKD_HOME", home.path())
        .env("APIS_BASE_URL", base_url)
        .env("APIS_ACCESS_TOKEN", "token-test")
        .args(args)
        .output()
        .expect("diskd should execute")
}

/// Converts process stdout bytes to UTF-8 for assertions.
fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be utf8")
}

/// Converts process stderr bytes to UTF-8 for diagnostics.
fn stderr_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be utf8")
}

/// Reads a tiny_http request body as JSON.
fn request_json(request: &mut Request) -> Value {
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .expect("request body should be readable");
    serde_json::from_str(&body).expect("request body should be json")
}

/// Reads a required request header value.
fn request_header(request: &Request, name: &str) -> String {
    request
        .headers()
        .iter()
        .find(|header| header.field.to_string().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().to_owned())
        .unwrap_or_default()
}

/// Sends a JSON response with application/json content type.
fn respond_json(request: Request, value: Value) {
    let header = Header::from_bytes("Content-Type", "application/json").unwrap();
    request
        .respond(Response::from_string(value.to_string()).with_header(header))
        .expect("response should send");
}

/// Sends a binary/text response body.
fn respond_bytes(request: Request, body: &'static [u8]) {
    request
        .respond(Response::from_data(body))
        .expect("response should send");
}

/// Creates a local file fixture and returns its path.
fn write_fixture_file(home: &TempDir, name: &str, contents: &[u8]) -> PathBuf {
    let path = home.path().join(name);
    std::fs::write(&path, contents).expect("fixture should be written");
    path
}

/* REQ-DISKD-CLI-015: ls must call the gateway Drive JSON-RPC endpoint with bearer auth, project-normalized paths, and ls-like text output using copyable names, display metadata, and indexing status. */
#[test]
fn ls_normalizes_project_path_and_uses_bearer_auth() {
    let gateway = start_gateway(1, |_, mut request| {
        assert_eq!(request.method().as_str(), "POST");
        assert_eq!(request.url(), "/v1/os/drive/api/v1");
        assert_eq!(
            request_header(&request, "Authorization"),
            "Bearer token-test"
        );
        let body = request_json(&mut request);
        assert_eq!(body["method"], "paths/tools/ls");
        assert_eq!(body["params"]["path"], "/Projects/01PROJECT/docs");
        respond_json(
            request,
            json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {
                    "entries": [
                        { "name": "reports", "metadata": { "displayName": "Reports" }, "type": "dir", "full_path": "/Projects/01PROJECT/docs/reports", "size": 0 },
                        { "name": "a.txt", "displayName": "A Document", "type": "file", "full_path": "/Projects/01PROJECT/docs/a.txt", "size": 5, "indexingStatus": "indexed" }
                    ]
                }
            }),
        );
    });
    let home = TempDir::new().unwrap();

    let output = run_diskd(
        &home,
        &gateway.base_url,
        &["--project", "01PROJECT", "ls", "docs"],
    );

    gateway.join();
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(
        stdout_text(&output),
        "<DIR>          0 -              reports (Reports)\n<FILE>         5 indexed        a.txt (A Document)\n"
    );
}

/* REQ-DISKD-CLI-033: tree -L must bound Drive traversal before rendering copyable names with display metadata. */
#[test]
fn tree_renders_recursive_ls_with_depth_and_sizes() {
    let gateway = start_gateway(2, |index, mut request| {
        assert_eq!(request.method().as_str(), "POST");
        assert_eq!(request.url(), "/v1/os/drive/api/v1");
        assert_eq!(
            request_header(&request, "Authorization"),
            "Bearer token-test"
        );
        let body = request_json(&mut request);
        assert_eq!(body["method"], "paths/tools/ls");
        assert_eq!(body["params"].get("recursive"), None);
        assert_eq!(body["params"]["show_hidden"], true);
        match index {
            0 => {
                assert_eq!(body["params"]["path"], "/Projects/01PROJECT/docs");
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "entries": [
                                { "name": "reports", "metadata": { "displayName": "Reports" }, "type": "dir", "full_path": "/Projects/01PROJECT/docs/reports", "size": 0 },
                                { "name": "a.txt", "displayName": "A Document", "type": "file", "full_path": "/Projects/01PROJECT/docs/a.txt", "size": 5 }
                            ]
                        }
                    }),
                );
            }
            1 => {
                assert_eq!(body["params"]["path"], "/Projects/01PROJECT/docs/reports");
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "entries": [
                                { "name": "q1.pdf", "displayName": "Q1 Report", "type": "file", "full_path": "/Projects/01PROJECT/docs/reports/q1.pdf", "size": 17 }
                            ]
                        }
                    }),
                );
            }
            _ => unreachable!("unexpected request index"),
        }
    });
    let home = TempDir::new().unwrap();

    let output = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--project",
            "01PROJECT",
            "tree",
            "docs",
            "-L",
            "2",
            "-s",
            "-a",
        ],
    );

    gateway.join();
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(
        stdout_text(&output),
        "docs\n|-- <FILE>        5 a.txt (A Document)\n`-- <DIR>        0 reports (Reports)\n    `-- <FILE>       17 q1.pdf (Q1 Report)\n"
    );
}

/* REQ-DISKD-CLI-016: set-context --list must call the platform projects REST route and print only id/name. */
#[test]
fn set_context_list_reads_platform_projects() {
    let gateway = start_gateway(1, |_, request| {
        assert_eq!(request.method().as_str(), "GET");
        assert_eq!(request.url(), "/v1/platform/projects/api/projects");
        assert_eq!(
            request_header(&request, "Authorization"),
            "Bearer token-test"
        );
        respond_json(
            request,
            json!([
                { "id": "01PROJECT", "name": "Alpha", "description": "ignored" },
                { "id": "02PROJECT", "name": "Beta" }
            ]),
        );
    });
    let home = TempDir::new().unwrap();

    let output = run_diskd(
        &home,
        &gateway.base_url,
        &["--json", "set-context", "--list"],
    );

    gateway.join();
    assert!(output.status.success(), "{}", stderr_text(&output));
    let printed: Value = serde_json::from_str(&stdout_text(&output)).unwrap();
    assert_eq!(printed[0], json!({ "id": "01PROJECT", "name": "Alpha" }));
}

/* REQ-DISKD-CLI-017: upload must execute Drive start, upload PUT, then commit with the returned intent and etag. */
#[test]
fn upload_runs_start_put_commit_sequence() {
    let gateway = start_gateway(3, |index, mut request| match index {
        0 => {
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/v1/os/drive/api/v1");
            let body = request_json(&mut request);
            assert_eq!(body["method"], "drive/upload/start");
            assert_eq!(body["params"]["name"], "note.txt");
            assert_eq!(body["params"]["parent_path"], "/Projects/01PROJECT");
            assert_eq!(body["params"]["size"], 5);
            respond_json(
                request,
                json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": {
                        "intent_id": "intent-1",
                        "inode": "inode-1",
                        "upload_url": "/upload/intent-1",
                        "expires_in": 60,
                        "multipart": false
                    }
                }),
            );
        }
        1 => {
            assert_eq!(request.method().as_str(), "PUT");
            assert_eq!(request.url(), "/v1/os/drive/upload/intent-1");
            assert_eq!(request_header(&request, "X-Upload-Intent-Id"), "intent-1");
            respond_json(request, json!({ "etag": "etag-1" }));
        }
        2 => {
            assert_eq!(request.method().as_str(), "POST");
            let body = request_json(&mut request);
            assert_eq!(body["method"], "drive/upload/commit");
            assert_eq!(body["params"]["intent_id"], "intent-1");
            assert_eq!(body["params"]["etag"], "etag-1");
            respond_json(
                request,
                json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": {
                        "inode": "inode-1",
                        "etag": "etag-1",
                        "version": 1,
                        "committed_at": "2026-07-05T00:00:00Z"
                    }
                }),
            );
        }
        _ => unreachable!(),
    });
    let home = TempDir::new().unwrap();
    let file = write_fixture_file(&home, "note.txt", b"hello");

    let output = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--project",
            "01PROJECT",
            "--json",
            "upload",
            file.to_str().unwrap(),
            "--dest",
            "/",
            "--force",
        ],
    );

    gateway.join();
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert!(stdout_text(&output).contains("inode-1"));
}

/* REQ-DISKD-CLI-018: cat must resolve a download URL through Drive JSON-RPC and stream the bytes to stdout. */
#[test]
fn cat_streams_downloaded_bytes() {
    let base_url_holder = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let base_url_for_handler = base_url_holder.clone();
    let gateway = start_gateway(2, move |index, mut request| match index {
        0 => {
            let base_url = base_url_for_handler.lock().unwrap().clone();
            assert_eq!(request.method().as_str(), "POST");
            let body = request_json(&mut request);
            assert_eq!(body["method"], "drive/files/download-url");
            assert_eq!(body["params"]["path"], "/Projects/01PROJECT/a.txt");
            respond_json(
                request,
                json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": { "url": format!("{base_url}/download/a.txt"), "expires_in": 60 }
                }),
            );
        }
        1 => {
            assert_eq!(request.method().as_str(), "GET");
            assert_eq!(request.url(), "/download/a.txt");
            respond_bytes(request, b"hello from drive");
        }
        _ => unreachable!(),
    });
    *base_url_holder.lock().unwrap() = gateway.base_url.clone();
    let home = TempDir::new().unwrap();

    let output = run_diskd(
        &home,
        &gateway.base_url,
        &["--project", "01PROJECT", "cat", "a.txt"],
    );

    gateway.join();
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(output.stdout, b"hello from drive");
}

/* REQ-DISKD-CLI-027: read must accept --limit/--offset aliases and send parts_limit/parts_offset to Drive. */
#[test]
fn read_accepts_limit_offset_aliases() {
    let gateway = start_gateway(1, |_, mut request| {
        assert_eq!(request.method().as_str(), "POST");
        let body = request_json(&mut request);
        assert_eq!(body["method"], "paths/tools/read");
        assert_eq!(
            body["params"]["path"],
            "/Projects/01PROJECT/docs/report.pdf"
        );
        assert_eq!(body["params"]["parts_limit"], 2);
        assert_eq!(body["params"]["parts_offset"], 4);
        respond_json(
            request,
            json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {
                    "parts": [],
                    "total_parts": 10,
                    "parts_offset": 4,
                    "next_offset": 6,
                    "eof": false
                }
            }),
        );
    });
    let home = TempDir::new().unwrap();

    let output = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--project",
            "01PROJECT",
            "--json",
            "read",
            "docs/report.pdf",
            "--limit",
            "2",
            "--offset",
            "4",
        ],
    );

    gateway.join();
    assert!(output.status.success(), "{}", stderr_text(&output));
    let printed: Value = serde_json::from_str(&stdout_text(&output)).unwrap();
    assert_eq!(printed["parts_offset"], 4);
}

/* REQ-DISKD-CLI-028: search commands must pass limit and offset through to Drive path search. */
#[test]
fn search_commands_pass_limit_and_offset() {
    let gateway = start_gateway(2, |index, mut request| {
        assert_eq!(request.method().as_str(), "POST");
        let body = request_json(&mut request);
        match index {
            0 => {
                assert_eq!(body["method"], "paths/tools/grep");
                assert_eq!(body["params"]["query"], "needle");
                assert_eq!(body["params"]["paths"], json!(["/Projects/01PROJECT/docs"]));
                assert_eq!(body["params"]["limit"], 3);
                assert_eq!(body["params"]["offset"], 6);
            }
            1 => {
                assert_eq!(body["method"], "paths/tools/vsearch");
                assert_eq!(body["params"]["query"], "semantic needle");
                assert_eq!(body["params"]["paths"], json!(["/Projects/01PROJECT/docs"]));
                assert_eq!(body["params"]["limit"], 4);
                assert_eq!(body["params"]["offset"], 8);
            }
            _ => unreachable!(),
        }
        respond_json(
            request,
            json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": { "documents": [] }
            }),
        );
    });
    let home = TempDir::new().unwrap();

    let grep = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--project",
            "01PROJECT",
            "--json",
            "grep",
            "needle",
            "docs",
            "--limit",
            "3",
            "--offset",
            "6",
        ],
    );
    let vsearch = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--project",
            "01PROJECT",
            "--json",
            "vsearch",
            "semantic needle",
            "docs",
            "--limit",
            "4",
            "--offset",
            "8",
        ],
    );

    gateway.join();
    assert!(grep.status.success(), "{}", stderr_text(&grep));
    assert!(vsearch.status.success(), "{}", stderr_text(&vsearch));
}

/* REQ-DISKD-CLI-030: database/db must expose generic Drive DB JSON-RPC methods with optional db_type. */
#[test]
fn database_commands_call_drive_db_api() {
    let home = TempDir::new().unwrap();
    let rows_file = write_fixture_file(&home, "db-rows.json", br#"[{"id":1,"text":"hello"}]"#);
    let gateway = start_gateway(4, |index, mut request| {
        assert_eq!(request.method().as_str(), "POST");
        assert_eq!(request.url(), "/v1/os/drive/api/v1");
        let body = request_json(&mut request);
        match index {
            0 => {
                assert_eq!(body["method"], "drive/db/create");
                assert_eq!(body["params"]["name"], "generic-db");
                assert_eq!(body["params"]["db_type"], "telegram");
                assert_eq!(
                    body["params"]["schema"]["items"][0],
                    "CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"
                );
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "db_inode": "db-inode-1",
                            "file_id": "file-1",
                            "name": "generic-db.telegram",
                            "status": "ready"
                        }
                    }),
                );
            }
            1 => {
                assert_eq!(body["method"], "drive/db/insert");
                assert_eq!(body["params"]["name"], "generic-db");
                assert_eq!(body["params"]["table"], "messages");
                assert_eq!(body["params"]["db_type"], "telegram");
                assert_eq!(
                    body["params"]["rows"],
                    json!([{ "id": 1, "text": "hello" }])
                );
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "inserted": 1,
                            "pending_rows": 1,
                            "status": "pending"
                        }
                    }),
                );
            }
            2 => {
                assert_eq!(body["method"], "drive/db/query");
                assert_eq!(body["params"]["name"], "generic-db");
                assert_eq!(body["params"]["db_type"], "telegram");
                assert_eq!(
                    body["params"]["sql"],
                    "SELECT id, text FROM messages WHERE text = ?"
                );
                assert_eq!(body["params"]["parameters"], json!(["hello"]));
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "rows": [{ "id": 1, "text": "hello" }]
                        }
                    }),
                );
            }
            3 => {
                assert_eq!(body["method"], "drive/db/resolve-with-settings");
                assert_eq!(body["params"]["db_inode"], "db-inode-1");
                assert_eq!(body["params"]["db_type"], "telegram");
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "name": "generic-db.telegram",
                            "db_inode": "db-inode-1",
                            "file_id": "file-1",
                            "status": "ready",
                            "db_type": "telegram",
                            "settings": { "title": "Generic DB" }
                        }
                    }),
                );
            }
            _ => unreachable!(),
        }
    });

    let create = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "database",
            "create",
            "generic-db",
            "--schema",
            r#"{"items":["CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"]}"#,
            "--db-type",
            "telegram",
        ],
    );
    let insert = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "db",
            "insert",
            "generic-db",
            "messages",
            "--rows-file",
            rows_file.to_str().unwrap(),
            "--db-type",
            "telegram",
        ],
    );
    let query = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "database",
            "query",
            "generic-db",
            "SELECT id, text FROM messages WHERE text = ?",
            "--parameters",
            r#"["hello"]"#,
            "--db-type",
            "telegram",
        ],
    );
    let resolve = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "db",
            "resolve-with-settings",
            "db-inode-1",
            "--db-type",
            "telegram",
        ],
    );

    gateway.join();
    assert!(create.status.success(), "{}", stderr_text(&create));
    assert!(insert.status.success(), "{}", stderr_text(&insert));
    assert!(query.status.success(), "{}", stderr_text(&query));
    assert!(resolve.status.success(), "{}", stderr_text(&resolve));
    let printed: Value = serde_json::from_str(&stdout_text(&resolve)).unwrap();
    assert_eq!(printed["settings"]["title"], "Generic DB");
}

/* REQ-DISKD-CLI-029: telegram-db must expose the Drive Telegram DB JSON-RPC API for create, insert, and query workflows. */
#[test]
fn telegram_db_commands_call_drive_telegram_api() {
    let home = TempDir::new().unwrap();
    let rows_file = write_fixture_file(&home, "rows.json", br#"[{"id":1,"text":"hello"}]"#);
    let gateway = start_gateway(3, |index, mut request| {
        assert_eq!(request.method().as_str(), "POST");
        assert_eq!(request.url(), "/v1/os/drive/api/v1");
        assert_eq!(
            request_header(&request, "Authorization"),
            "Bearer token-test"
        );
        let body = request_json(&mut request);
        match index {
            0 => {
                assert_eq!(body["method"], "drive/telegram/create");
                assert_eq!(body["params"]["name"], "team-chat");
                assert_eq!(
                    body["params"]["schema"]["items"][0],
                    "CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"
                );
                assert_eq!(body["params"]["recreate"], true);
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "db_inode": "db-inode-1",
                            "file_id": "file-1",
                            "name": "team-chat.telegram",
                            "status": "ready"
                        }
                    }),
                );
            }
            1 => {
                assert_eq!(body["method"], "drive/telegram/insert");
                assert_eq!(body["params"]["name"], "team-chat");
                assert_eq!(body["params"]["table"], "messages");
                assert_eq!(
                    body["params"]["rows"],
                    json!([{ "id": 1, "text": "hello" }])
                );
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "inserted": 1,
                            "pending_rows": 1,
                            "status": "pending"
                        }
                    }),
                );
            }
            2 => {
                assert_eq!(body["method"], "drive/telegram/query");
                assert_eq!(body["params"]["name"], "team-chat");
                assert_eq!(
                    body["params"]["sql"],
                    "SELECT id, text FROM messages WHERE text = ?"
                );
                assert_eq!(body["params"]["parameters"], json!(["hello"]));
                respond_json(
                    request,
                    json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "rows": [{ "id": 1, "text": "hello" }]
                        }
                    }),
                );
            }
            _ => unreachable!(),
        }
    });

    let create = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "telegram-db",
            "create",
            "team-chat",
            "--schema",
            r#"{"items":["CREATE TABLE messages (id INTEGER PRIMARY KEY, text TEXT)"]}"#,
            "--recreate",
        ],
    );
    let insert = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "telegram-db",
            "insert",
            "team-chat",
            "messages",
            "--rows-file",
            rows_file.to_str().unwrap(),
        ],
    );
    let query = run_diskd(
        &home,
        &gateway.base_url,
        &[
            "--json",
            "telegram-db",
            "query",
            "team-chat",
            "SELECT id, text FROM messages WHERE text = ?",
            "--parameters",
            r#"["hello"]"#,
        ],
    );

    gateway.join();
    assert!(create.status.success(), "{}", stderr_text(&create));
    assert!(insert.status.success(), "{}", stderr_text(&insert));
    assert!(query.status.success(), "{}", stderr_text(&query));
    let printed: Value = serde_json::from_str(&stdout_text(&query)).unwrap();
    assert_eq!(printed["rows"][0]["text"], "hello");
}
