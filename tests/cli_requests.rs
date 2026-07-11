use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::Duration,
};

type RequestCase<'a> = (&'a [&'a str], &'a str, Option<(&'a str, &'a str)>);

#[test]
fn typed_task_create_sends_expected_asana_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("server address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let request = read_http_request(&mut stream);
        let response = r#"{"data":{"gid":"task-1","name":"Example"}}"#;
        write!(
            stream,
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(),
            response
        )
        .expect("write response");
        request
    });

    let config_home = std::env::temp_dir().join(format!(
        "asana-cli-test-{}-{}",
        std::process::id(),
        address.port()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_asana"))
        .env("ASANA_ACCESS_TOKEN", "test-token")
        .env("ASANA_API_BASE_URL", format!("http://{address}/api/1.0/"))
        .env("XDG_CONFIG_HOME", &config_home)
        .args([
            "--json",
            "--workspace",
            "workspace-1",
            "task",
            "create",
            "--name",
            "Example",
            "--tag",
            "tag-1",
        ])
        .output()
        .expect("run asana CLI");
    let _ = std::fs::remove_dir_all(config_home);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("JSON CLI output");
    assert_eq!(stdout["gid"], "task-1");

    let request = server.join().expect("server thread");
    assert!(request.starts_with("POST /api/1.0/tasks HTTP/1.1\r\n"));
    assert!(request.contains("authorization: Bearer test-token\r\n"));
    let (_, body) = request.split_once("\r\n\r\n").expect("request body");
    let body: serde_json::Value = serde_json::from_str(body).expect("JSON request body");
    assert_eq!(body["data"]["name"], "Example");
    assert_eq!(body["data"]["workspace"], "workspace-1");
    assert_eq!(body["data"]["tags"][0], "tag-1");
}

#[test]
fn phase_two_typed_writes_use_official_paths_and_data_envelopes() {
    let cases: &[RequestCase<'_>] = &[
        (
            &[
                "--workspace",
                "ws",
                "project",
                "create",
                "--name",
                "Roadmap",
                "--team",
                "team-1",
            ],
            "POST /api/1.0/teams/team-1/projects HTTP/1.1\r\n",
            Some(("name", "Roadmap")),
        ),
        (
            &[
                "--workspace",
                "ws",
                "status-update",
                "create",
                "--parent",
                "project-1",
                "--status-type",
                "on_track",
                "--text",
                "Good",
            ],
            "POST /api/1.0/status_updates HTTP/1.1\r\n",
            Some(("parent", "project-1")),
        ),
        (
            &[
                "--workspace",
                "ws",
                "project-template",
                "instantiate",
                "template-1",
                "--name",
                "Launch",
            ],
            "POST /api/1.0/project_templates/template-1/instantiateProject HTTP/1.1\r\n",
            Some(("name", "Launch")),
        ),
        (
            &[
                "--workspace",
                "ws",
                "membership",
                "create",
                "--parent",
                "project-1",
                "--member",
                "user-1",
                "--access-level",
                "editor",
            ],
            "POST /api/1.0/memberships HTTP/1.1\r\n",
            Some(("member", "user-1")),
        ),
    ];

    for (args, expected_line, expected_field) in cases {
        let request = capture_request(args);
        assert!(request.starts_with(expected_line), "request was: {request}");
        let (_, body) = request.split_once("\r\n\r\n").expect("request body");
        let body: serde_json::Value = serde_json::from_str(body).expect("JSON request body");
        if let Some((key, value)) = expected_field {
            assert_eq!(body["data"][key], *value);
        }
    }
}

#[test]
fn access_request_decision_posts_without_a_fabricated_body() {
    let request = capture_request(&[
        "--profile",
        "admin",
        "--workspace",
        "ws",
        "--yes",
        "access-request",
        "approve",
        "request-1",
    ]);
    assert!(request.starts_with("POST /api/1.0/access_requests/request-1/approve HTTP/1.1\r\n"));
    let (_, body) = request.split_once("\r\n\r\n").expect("request separator");
    assert!(body.is_empty(), "approval endpoint must not receive a body");
}

#[test]
fn every_phase_two_mutation_normalizes_to_its_official_endpoint() {
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["workspace", "update", "ws", "--name", "Renamed"],
            "PUT",
            "workspaces/ws",
        ),
        (
            &["workspace", "user-add", "ws", "--user", "u1"],
            "POST",
            "workspaces/ws/addUser",
        ),
        (
            &["workspace", "user-remove", "ws", "--user", "u1"],
            "POST",
            "workspaces/ws/removeUser",
        ),
        (
            &["project", "update", "p1", "--name", "Renamed"],
            "PUT",
            "projects/p1",
        ),
        (&["project", "delete", "p1"], "DELETE", "projects/p1"),
        (
            &["project", "duplicate", "p1", "--name", "Copy"],
            "POST",
            "projects/p1/duplicate",
        ),
        (
            &["project", "member", "p1", "--add", "u1"],
            "POST",
            "projects/p1/addMembers",
        ),
        (
            &["project", "member", "p1", "--remove", "u1"],
            "POST",
            "projects/p1/removeMembers",
        ),
        (
            &["project", "follower", "p1", "--add", "u1"],
            "POST",
            "projects/p1/addFollowers",
        ),
        (
            &["project", "custom-field", "p1", "cf1"],
            "POST",
            "projects/p1/addCustomFieldSetting",
        ),
        (
            &["project", "custom-field", "p1", "cf1", "--remove"],
            "POST",
            "projects/p1/removeCustomFieldSetting",
        ),
        (
            &[
                "project",
                "brief",
                "create",
                "--project",
                "p1",
                "--title",
                "Brief",
            ],
            "POST",
            "projects/p1/project_briefs",
        ),
        (
            &["project", "brief", "update", "b1", "--title", "Brief"],
            "PUT",
            "project_briefs/b1",
        ),
        (
            &["project", "brief", "delete", "b1"],
            "DELETE",
            "project_briefs/b1",
        ),
        (
            &[
                "project",
                "save-as-template",
                "p1",
                "--name",
                "Template",
                "--private",
            ],
            "POST",
            "projects/p1/saveAsTemplate",
        ),
        (
            &["project-template", "delete", "pt1"],
            "DELETE",
            "project_templates/pt1",
        ),
        (
            &[
                "project-template",
                "instantiate",
                "pt1",
                "--name",
                "Project",
            ],
            "POST",
            "project_templates/pt1/instantiateProject",
        ),
        (
            &["task-template", "delete", "tt1"],
            "DELETE",
            "task_templates/tt1",
        ),
        (
            &["task-template", "instantiate", "tt1"],
            "POST",
            "task_templates/tt1/instantiateTask",
        ),
        (
            &["user", "update", "u1", "--name", "New name"],
            "PUT",
            "users/u1",
        ),
        (
            &[
                "user",
                "update",
                "u1",
                "--in-workspace",
                "--data",
                "{\"custom_fields\":{}}",
            ],
            "PUT",
            "workspaces/ws/users/u1",
        ),
        (&["team", "create", "--name", "Team"], "POST", "teams"),
        (
            &["team", "update", "t1", "--name", "Team"],
            "PUT",
            "teams/t1",
        ),
        (
            &["team", "user-add", "t1", "--user", "u1"],
            "POST",
            "teams/t1/addUser",
        ),
        (
            &["team", "user-remove", "t1", "--user", "u1"],
            "POST",
            "teams/t1/removeUser",
        ),
        (
            &["membership", "create", "--parent", "p1", "--member", "u1"],
            "POST",
            "memberships",
        ),
        (
            &["membership", "update", "m1", "--access-level", "editor"],
            "PUT",
            "memberships/m1",
        ),
        (&["membership", "delete", "m1"], "DELETE", "memberships/m1"),
        (
            &["access-request", "create", "--target", "p1"],
            "POST",
            "access_requests",
        ),
        (
            &["access-request", "approve", "a1"],
            "POST",
            "access_requests/a1/approve",
        ),
        (
            &["access-request", "reject", "a1"],
            "POST",
            "access_requests/a1/reject",
        ),
        (
            &[
                "status-update",
                "create",
                "--parent",
                "p1",
                "--status-type",
                "on_track",
                "--text",
                "OK",
            ],
            "POST",
            "status_updates",
        ),
        (
            &["status-update", "delete", "s1"],
            "DELETE",
            "status_updates/s1",
        ),
    ];

    for (args, method, path) in cases {
        let preview = dry_run_preview(args);
        assert_eq!(preview["method"], *method, "args: {args:?}");
        assert_eq!(preview["path"], *path, "args: {args:?}");
    }
}

#[test]
fn phase_two_reads_use_official_resource_endpoints() {
    let cases: &[(&[&str], &str)] = &[
        (
            &["--workspace", "ws", "project", "list", "--team", "t1"],
            "GET /api/1.0/teams/t1/projects?",
        ),
        (
            &["--workspace", "ws", "project", "list", "--task", "task1"],
            "GET /api/1.0/tasks/task1/projects?",
        ),
        (
            &["--workspace", "ws", "project", "search", "Roadmap"],
            "GET /api/1.0/workspaces/ws/projects/search?",
        ),
        (
            &["project", "get", "p1"],
            "GET /api/1.0/projects/p1 HTTP/1.1\r\n",
        ),
        (
            &["project", "task-counts", "p1"],
            "GET /api/1.0/projects/p1/task_counts HTTP/1.1\r\n",
        ),
        (
            &["project", "custom-fields", "p1"],
            "GET /api/1.0/projects/p1/custom_field_settings?",
        ),
        (
            &["project", "brief", "get", "p1"],
            "GET /api/1.0/projects/p1/project_briefs HTTP/1.1\r\n",
        ),
        (
            &["--workspace", "ws", "project-template", "list"],
            "GET /api/1.0/project_templates?",
        ),
        (
            &["project-template", "get", "pt1"],
            "GET /api/1.0/project_templates/pt1 HTTP/1.1\r\n",
        ),
        (
            &["task-template", "list", "--project", "p1"],
            "GET /api/1.0/task_templates?",
        ),
        (
            &["task-template", "get", "tt1"],
            "GET /api/1.0/task_templates/tt1 HTTP/1.1\r\n",
        ),
        (
            &["--workspace", "ws", "user", "list"],
            "GET /api/1.0/workspaces/ws/users?",
        ),
        (&["user", "get", "u1"], "GET /api/1.0/users/u1 HTTP/1.1\r\n"),
        (
            &["--workspace", "ws", "user", "get", "u1", "--in-workspace"],
            "GET /api/1.0/workspaces/ws/users/u1 HTTP/1.1\r\n",
        ),
        (
            &["--workspace", "ws", "user", "favorites", "u1"],
            "GET /api/1.0/users/u1/favorites?",
        ),
        (
            &["--workspace", "ws", "team", "list"],
            "GET /api/1.0/workspaces/ws/teams?",
        ),
        (&["team", "get", "t1"], "GET /api/1.0/teams/t1 HTTP/1.1\r\n"),
        (
            &["membership", "list", "--parent", "p1"],
            "GET /api/1.0/memberships?",
        ),
        (
            &["membership", "get", "m1"],
            "GET /api/1.0/memberships/m1 HTTP/1.1\r\n",
        ),
        (
            &["membership", "project", "--parent", "p1"],
            "GET /api/1.0/projects/p1/project_memberships?",
        ),
        (
            &["membership", "team", "--user", "u1"],
            "GET /api/1.0/users/u1/team_memberships?",
        ),
        (
            &["membership", "workspace", "--parent", "ws"],
            "GET /api/1.0/workspaces/ws/workspace_memberships?",
        ),
        (
            &["access-request", "list", "--target", "p1"],
            "GET /api/1.0/access_requests?",
        ),
        (
            &["status-update", "list", "--parent", "p1"],
            "GET /api/1.0/status_updates?",
        ),
        (
            &["status-update", "get", "s1"],
            "GET /api/1.0/status_updates/s1 HTTP/1.1\r\n",
        ),
        (
            &["workspace", "get", "ws"],
            "GET /api/1.0/workspaces/ws HTTP/1.1\r\n",
        ),
    ];

    for (args, expected_prefix) in cases {
        let request = capture_request(args);
        assert!(
            request.starts_with(expected_prefix),
            "args: {args:?}; request: {request}"
        );
    }
}

fn dry_run_preview(args: &[&str]) -> serde_json::Value {
    let config_home = std::env::temp_dir().join(format!(
        "asana-cli-dry-run-test-{}-{}",
        std::process::id(),
        args.join("-").len()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_asana"))
        .env_remove("ASANA_ACCESS_TOKEN")
        .env("XDG_CONFIG_HOME", &config_home)
        .args([
            "--json",
            "--dry-run",
            "--profile",
            "test",
            "--workspace",
            "ws",
        ])
        .args(args)
        .output()
        .expect("run dry-run command");
    let _ = std::fs::remove_dir_all(config_home);
    assert!(
        output.status.success(),
        "args: {args:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON dry-run preview")
}

fn capture_request(args: &[&str]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("server address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let request = read_http_request(&mut stream);
        let response = r#"{"data":{"gid":"result-1"}}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).expect("write response");
        request
    });
    let config_home = std::env::temp_dir().join(format!(
        "asana-cli-phase2-test-{}-{}",
        std::process::id(),
        address.port()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_asana"))
        .env("ASANA_ACCESS_TOKEN", "test-token")
        .env("ASANA_API_BASE_URL", format!("http://{address}/api/1.0/"))
        .env("XDG_CONFIG_HOME", &config_home)
        .arg("--json")
        .args(args)
        .output()
        .expect("run asana CLI");
    let _ = std::fs::remove_dir_all(config_home);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().expect("server thread")
}

fn read_http_request(stream: &mut impl Read) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end;
    loop {
        let count = stream.read(&mut buffer).expect("read request");
        assert!(count > 0, "connection closed before headers");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
    }
    let headers = String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut buffer).expect("read request body");
        assert!(count > 0, "connection closed before request body");
        bytes.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8(bytes).expect("UTF-8 HTTP request")
}
