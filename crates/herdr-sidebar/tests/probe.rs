use serde_json::Value;
use std::{path::PathBuf, process::Command, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
};

#[tokio::test]
async fn probe_reads_only_a_snapshot_and_exits_without_a_husk_host() {
    let path = PathBuf::from(format!(
        "/tmp/dolphin-probe-test-{}.sock",
        std::process::id()
    ));
    let listener = UnixListener::bind(&path).unwrap();
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../dolphin-herdr/tests/fixtures/snapshot.json"
    )))
    .unwrap();
    let snapshot = fixture["result"]["snapshot"].clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["method"], "session.snapshot");
        assert_eq!(request["params"], serde_json::json!({}));
        let mut reply = fixture;
        reply["id"] = request["id"].clone();
        let mut bytes = serde_json::to_vec(&reply).unwrap();
        bytes.push(b'\n');
        reader.get_mut().write_all(&bytes).await.unwrap();
    });
    let child_path = path.clone();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_herdr-sidebar"))
            .arg("--probe")
            .env("HERDR_SOCKET_PATH", child_path)
            .env_remove("HUSK_SOCKET_PATH")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 3);
    assert!(stdout.contains(&format!(
        "{} workspaces",
        snapshot["workspaces"].as_array().unwrap().len()
    )));
    assert!(stdout.contains(&format!(
        "{} agents",
        snapshot["agents"].as_array().unwrap().len()
    )));
    assert!(stdout.starts_with("Herdr v"));
}
