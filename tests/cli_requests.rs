use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::Duration,
};

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
