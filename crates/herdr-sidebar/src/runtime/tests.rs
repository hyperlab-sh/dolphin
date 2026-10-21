use super::*;
use dolphin_herdr::{AgentInfo, PaneInfo};
use husk_plugin::{
    read_line, sizes, CommandInvokePayload, SlotKeyPayload, SlotResizePayload, SIDEBAR_FOOTER,
    SIDEBAR_LIST,
};
use serde_json::{json, Value};
use std::{
    future::Future,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{
    io::{
        AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf,
    },
    net::{UnixListener, UnixStream},
    sync::Notify,
};

struct Socket(PathBuf);
impl Socket {
    fn bind() -> (Self, UnixListener) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = PathBuf::from(format!(
            "/tmp/dolphin-actor-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).unwrap();
        (Self(path), listener)
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn serve<F, Fut>(listener: UnixListener, handler: F) -> JoinSet<()>
where
    F: Fn(UnixStream) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let handler = Arc::new(handler);
    let mut server = JoinSet::new();
    server.spawn(async move {
        let mut clients = JoinSet::new();
        loop {
            tokio::select! {
                stream = listener.accept() => {
                    let (stream, _) = stream.unwrap();
                    clients.spawn(handler(stream));
                }
                result = clients.join_next(), if !clients.is_empty() => { result.unwrap().unwrap(); }
            }
        }
    });
    server
}
async fn stop_server(server: &mut JoinSet<()>) {
    server.abort_all();
    while server.join_next().await.is_some() {}
}
async fn request(stream: UnixStream) -> Option<(BufReader<UnixStream>, Value)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).await.unwrap() == 0 {
        return None;
    }
    let req = serde_json::from_str(&line).unwrap();
    Some((reader, req))
}
async fn write(reader: &mut BufReader<UnixStream>, value: Value) {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    // A superseded snapshot/layout may close its socket before this reply is written.
    let _ = reader.get_mut().write_all(&bytes).await;
}
async fn reply(reader: &mut BufReader<UnixStream>, req: &Value, result: Value) {
    write(reader, json!({"id":req["id"],"result":result})).await;
}
fn snapshot(label: &str) -> SessionSnapshot {
    let mut snapshot = crate::tests::sidebar().snapshot.unwrap();
    snapshot.version = "test".into();
    snapshot.workspaces[0].label = label.into();
    snapshot.panes.push(PaneInfo {
        pane_id: "p2".into(),
        workspace_id: "w1".into(),
        tab_id: "t1".into(),
        ..Default::default()
    });
    snapshot.agents.push(AgentInfo {
        pane_id: "p2".into(),
        workspace_id: "w1".into(),
        tab_id: "t1".into(),
        agent_status: "working".into(),
        ..Default::default()
    });
    snapshot
}
fn layout(pane: &str) -> Value {
    json!({"layout":{"workspace_id":"w1","tab_id":"t1","area":{"width":80,"height":24},"panes":[{"pane_id":pane,"focused":true,"rect":{"width":80,"height":24}}]}})
}
struct Host {
    reader: BufReader<ReadHalf<DuplexStream>>,
    writer: WriteHalf<DuplexStream>,
    runner: JoinSet<Result<()>>,
}
impl Host {
    fn start(socket: &Socket) -> Self {
        let (host, plugin) = tokio::io::duplex(1024 * 1024);
        let (plugin_read, plugin_write) = tokio::io::split(plugin);
        let (host_read, host_write) = tokio::io::split(host);
        let client = Client::new(socket.0.clone());
        let sidebar = Sidebar::new(socket.0.to_string_lossy().into_owned(), "self".into());
        let mut runner = JoinSet::new();
        runner.spawn(run_inner(
            sidebar,
            client,
            Box::new(BufReader::new(plugin_read)),
            Box::new(plugin_write),
            Duration::from_millis(1),
        ));
        Self {
            reader: BufReader::new(host_read),
            writer: host_write,
            runner,
        }
    }
    async fn until(&mut self, predicate: impl Fn(&Envelope) -> bool) -> Envelope {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let bytes = read_line(&mut self.reader, 4 * 1024 * 1024)
                    .await
                    .unwrap()
                    .expect("plugin must still be running");
                let envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
                if predicate(&envelope) {
                    return envelope;
                }
            }
        })
        .await
        .expect("UI must respond before a blocked RPC's three-second deadline")
    }
    async fn send(&mut self, envelope: Envelope) {
        send(&mut self.writer, &envelope).await.unwrap();
    }
    async fn stop(&mut self) {
        self.writer.shutdown().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), self.runner.join_next())
            .await
            .expect("host EOF must cancel pending RPCs immediately")
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(self.runner.is_empty());
    }
}
fn context(envelope: &Envelope, label: &str) -> bool {
    envelope
        .workspace_context
        .as_ref()
        .is_some_and(|p| p.workspace_label == label)
}
fn healthy(envelope: &Envelope) -> bool {
    envelope.slot_update.as_ref().is_some_and(|p| {
        p.slot == SIDEBAR_FOOTER && p.lines.iter().any(|l| l.contains("herdr vtest"))
    })
}
fn offline(envelope: &Envelope) -> bool {
    envelope
        .slot_update
        .as_ref()
        .is_some_and(|p| p.slot == SIDEBAR_LIST && p.badge == "OFFLINE")
}

#[tokio::test]
async fn quiet_reconnect_resyncs_after_startup_failure_and_eof() {
    let (socket, listener) = Socket::bind();
    let attempts = Arc::new(AtomicUsize::new(0));
    let version = Arc::new(AtomicUsize::new(1));
    let disconnect = Arc::new(Notify::new());
    let mut server = serve(listener, {
        let attempts = attempts.clone();
        let version = version.clone();
        let disconnect = disconnect.clone();
        move |stream| {
            let attempts = attempts.clone();
            let version = version.clone();
            let disconnect = disconnect.clone();
            async move {
                let Some((mut reader, req)) = request(stream).await else {
                    return;
                };
                match req["method"].as_str().unwrap() {
                    "events.subscribe" => {
                        let n = attempts.fetch_add(1, Ordering::SeqCst);
                        if n == 0 {
                            write(&mut reader,json!({"id":req["id"],"error":{"code":"unavailable","message":"starting"}})).await;
                            return;
                        }
                        reply(&mut reader, &req, json!({})).await;
                        if n == 1 {
                            disconnect.notified().await;
                        } else {
                            let _ = reader.read_u8().await;
                        }
                        // No events are emitted, before or after reconnect.
                    }
                    "session.snapshot" => {
                        if attempts.load(Ordering::SeqCst) < 2 {
                            write(&mut reader,json!({"id":req["id"],"error":{"code":"unavailable","message":"starting"}})).await;
                        } else {
                            reply(&mut reader,&req,json!({"snapshot":snapshot(&format!("quiet-{}",version.load(Ordering::SeqCst)))})).await;
                        }
                    }
                    "pane.layout" => {
                        reply(
                            &mut reader,
                            &req,
                            layout(req["params"]["pane_id"].as_str().unwrap()),
                        )
                        .await
                    }
                    method => panic!("unexpected mock method {method}"),
                }
            }
        }
    });
    let mut host = Host::start(&socket);
    host.until(offline).await;
    host.until(|e| context(e, "quiet-1")).await;
    host.until(healthy).await;
    version.store(2, Ordering::SeqCst);
    disconnect.notify_one();
    host.until(offline).await;
    host.until(|e| context(e, "quiet-2")).await;
    host.until(healthy).await;
    assert!(attempts.load(Ordering::SeqCst) >= 3);
    host.stop().await;
    stop_server(&mut server).await;
}

#[tokio::test]
async fn subscription_ack_closes_the_initial_snapshot_race() {
    let (socket, listener) = Socket::bind();
    let ack = Arc::new(Notify::new());
    let version = Arc::new(AtomicUsize::new(1));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut server = serve(listener, {
        let ack = ack.clone();
        let version = version.clone();
        let calls = calls.clone();
        move |stream| {
            let ack = ack.clone();
            let version = version.clone();
            let calls = calls.clone();
            async move {
                let Some((mut reader, req)) = request(stream).await else {
                    return;
                };
                match req["method"].as_str().unwrap() {
                    "events.subscribe" => {
                        ack.notified().await;
                        reply(&mut reader, &req, json!({})).await;
                        let _ = reader.read_u8().await;
                    }
                    "session.snapshot" => {
                        calls.fetch_add(1, Ordering::SeqCst);
                        reply(&mut reader,&req,json!({"snapshot":snapshot(if version.load(Ordering::SeqCst)==1 {"before-ack"} else {"after-ack"})})).await;
                    }
                    "pane.layout" => {
                        reply(
                            &mut reader,
                            &req,
                            layout(req["params"]["pane_id"].as_str().unwrap()),
                        )
                        .await
                    }
                    method => panic!("unexpected mock method {method}"),
                }
            }
        }
    });
    let mut host = Host::start(&socket);
    host.until(|e| context(e, "before-ack")).await;
    version.store(2, Ordering::SeqCst);
    ack.notify_one();
    host.until(|e| context(e, "after-ack")).await;
    host.until(healthy).await;
    assert!(calls.load(Ordering::SeqCst) >= 2);
    host.stop().await;
    stop_server(&mut server).await;
}

#[tokio::test]
async fn slow_layout_and_focus_never_block_selection_resize_or_host_close() {
    let (socket, listener) = Socket::bind();
    let (notice_tx, mut notices) = mpsc::unbounded_channel::<(String, String)>();
    let mut server = serve(listener, move |stream| {
        let notice_tx = notice_tx.clone();
        async move {
            let Some((mut reader, req)) = request(stream).await else {
                return;
            };
            let method = req["method"].as_str().unwrap();
            match method {
                "session.snapshot" => {
                    reply(
                        &mut reader,
                        &req,
                        json!({"snapshot":snapshot("slow-layout")}),
                    )
                    .await
                }
                "events.subscribe" | "pane.layout" | "agent.focus" => {
                    if method == "events.subscribe" {
                        reply(&mut reader, &req, json!({})).await;
                    }
                    let tag = if method == "pane.layout" {
                        req["params"]["pane_id"].as_str().unwrap()
                    } else {
                        method
                    };
                    notice_tx.send((tag.into(), "started".into())).unwrap();
                    // Deliberately never answer layout/focus: only cancellation can close these sockets.
                    assert!(reader.read_u8().await.is_err());
                    let _ = notice_tx.send((tag.into(), "closed".into()));
                }
                method => panic!("unexpected mock method {method}"),
            }
        }
    });
    let mut host = Host::start(&socket);
    host.until(healthy).await;
    host.send(Envelope {
        kind: "slot_key".into(),
        slot_key: Some(SlotKeyPayload {
            key: "G".into(),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await;
    host.until(|e| {
        e.workspace_context
            .as_ref()
            .is_some_and(|p| p.pane_id == "p2")
    })
    .await;
    host.send(Envelope {
        kind: "command_invoke".into(),
        command_invoke: Some(CommandInvokePayload {
            command_id: "herdr.focus_selected".into(),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await;
    let mut layout_started = false;
    let mut focus_started = false;
    tokio::time::timeout(Duration::from_secs(1), async {
        while !layout_started || !focus_started {
            let (tag, phase) = notices.recv().await.unwrap();
            layout_started |= tag == "p2" && phase == "started";
            focus_started |= tag == "agent.focus" && phase == "started";
        }
    })
    .await
    .unwrap();
    host.send(Envelope {
        kind: "slot_resize".into(),
        slot_resize: Some(SlotResizePayload {
            slot_sizes: sizes(&[(SIDEBAR_LIST, 26, 6)]),
            focused_slot: SIDEBAR_LIST.into(),
        }),
        ..Default::default()
    })
    .await;
    host.until(|e| {
        e.slot_update
            .as_ref()
            .is_some_and(|p| p.slot == SIDEBAR_LIST && p.lines.len() == 6)
    })
    .await;
    host.stop().await;
    let mut layout_closed = false;
    let mut focus_closed = false;
    let mut subscription_closed = false;
    tokio::time::timeout(Duration::from_secs(1), async {
        while !layout_closed || !focus_closed || !subscription_closed {
            let (tag, phase) = notices.recv().await.unwrap();
            layout_closed |= tag == "p2" && phase == "closed";
            focus_closed |= tag == "agent.focus" && phase == "closed";
            subscription_closed |= tag == "events.subscribe" && phase == "closed";
        }
    })
    .await
    .expect("host close must close subscription, layout and mutation sockets");
    stop_server(&mut server).await;
}

#[test]
fn superseded_layouts_and_pre_disconnect_snapshots_are_ignored() {
    let sidebar = crate::tests::sidebar();
    let mut controller = Controller::new(sidebar, Client::new("/tmp/not-used.sock"));
    controller.jobs.preview_generation = 7;
    controller.complete(Completion::Preview {
        generation: 6,
        pane: "p1".into(),
        result: Ok(PaneLayoutResult::default()),
    });
    assert!(controller.sidebar.preview_layout.is_none());
    controller.complete(Completion::Preview {
        generation: 7,
        pane: "p2".into(),
        result: Ok(PaneLayoutResult::default()),
    });
    assert!(controller.sidebar.preview_layout.is_none());
    controller.connection(Connection::Disconnected("gone".into()));
    controller.complete(Completion::Snapshot {
        generation: 0,
        result: Ok(snapshot("stale")),
    });
    assert!(!controller.sidebar.connected);
    assert_ne!(
        controller.sidebar.snapshot.as_ref().unwrap().workspaces[0].label,
        "stale"
    );
}
