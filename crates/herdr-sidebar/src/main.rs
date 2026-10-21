use anyhow::{bail, Context, Result};
use dolphin_herdr::Client;
use herdr_sidebar::{hello, Effect, Sidebar, FRAMES};
use husk_plugin::{read_loop, send, Envelope, Writer};
use std::{
    env,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        println!("herdr-sidebar — Husk plugin for Herdr\n\nUsage: herdr-sidebar [--probe]\n\n  --probe  Print a read-only live session summary and exit\n\nUses HUSK_SOCKET_PATH, or stdin/stdout when unset.\nHerdr: HERDR_SOCKET_PATH, XDG_CONFIG_HOME/herdr/herdr.sock, or ~/.config/herdr/herdr.sock.");
        return Ok(());
    }
    let client = Client::default();
    if args == ["--probe"] {
        let snap = client.snapshot().await.context("Could not read Herdr. Ensure the server is running and HERDR_SOCKET_PATH points to its socket; see --help")?;
        println!(
            "Herdr v{} · protocol {} · session {}",
            snap.version, snap.protocol, snap.session_name
        );
        println!(
            "{} workspaces · {} tabs · {} panes · {} agents",
            snap.workspaces.len(),
            snap.tabs.len(),
            snap.panes.len(),
            snap.agents.len()
        );
        println!(
            "Focused: workspace {} · tab {} · pane {}",
            snap.focused_workspace_id, snap.focused_tab_id, snap.focused_pane_id
        );
        return Ok(());
    }
    if !args.is_empty() {
        bail!("unknown arguments; use --help");
    }
    let (mut reader, mut writer) = husk_plugin::connect().await?;
    send(&mut writer, &hello()).await?;
    let mut sidebar = Sidebar::new(
        client.socket_path.to_string_lossy().into_owned(),
        env::var("HERDR_PANE_ID").unwrap_or_default(),
    );
    refresh(&mut sidebar, &client, true).await;
    flush(&mut sidebar, &mut writer).await?;

    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let reader_task = tokio::spawn(async move {
        let result = read_loop(&mut reader, |env| {
            let _ = host_tx.send(Ok(env));
        })
        .await;
        if let Err(error) = result {
            let _ = host_tx.send(Err(error));
        }
    });
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let event_client = client.clone();
    let events_task = tokio::spawn(async move {
        loop {
            if let Ok(mut stream) = event_client.subscribe_events().await {
                while let Ok(Some(event)) = stream.next_event().await {
                    if event_tx.send(event).await.is_err() {
                        return;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
    });
    let mut ticker = tokio::time::interval(Duration::from_millis(220));
    let mut last_snapshot = Instant::now() - Duration::from_secs(1);
    let result: Result<()> = async {
        loop {
            tokio::select! {
                host = host_rx.recv() => {
                    let Some(host) = host else {break};
                    let env: Envelope = host?;
                    let effects = sidebar.handle_host(env);
                    for effect in effects {apply(&mut sidebar, &client, effect).await;}
                }
                event = event_rx.recv() => {
                    if let Some(event) = event {
                        if event.pane_id == sidebar.self_pane_id && !event.pane_id.is_empty() && event.event == "pane.updated" {continue}
                        if last_snapshot.elapsed() < Duration::from_millis(150) {continue}
                        last_snapshot = Instant::now();
                        sidebar.event_count += 1; sidebar.last_event = event.event;
                        refresh(&mut sidebar, &client, false).await;
                    }
                }
                _ = ticker.tick() => {
                    sidebar.spinner_frame = (sidebar.spinner_frame + 1) % FRAMES.len();
                    if sidebar.snapshot.as_ref().is_some_and(|s|s.agents.iter().any(|a|a.agent_status == "working")) {sidebar.render_list();}
                }
            }
            flush(&mut sidebar, &mut writer).await?;
        }
        Ok(())
    }.await;
    // No detached tasks or subscription sockets survive host EOF or write errors.
    reader_task.abort();
    events_task.abort();
    let _ = reader_task.await;
    let _ = events_task.await;
    result
}
async fn flush(sidebar: &mut Sidebar, writer: &mut Writer) -> Result<()> {
    for message in sidebar.outbox.drain(..) {
        send(writer, &message).await?;
    }
    Ok(())
}
async fn preview(sidebar: &mut Sidebar, client: &Client) {
    let pane = sidebar.target_pane();
    if pane.is_empty() {
        sidebar.preview_pane_id.clear();
        sidebar.preview_layout = None;
        return;
    }
    sidebar.preview_layout = client.pane_layout(&pane).await.ok();
    sidebar.preview_pane_id = pane;
    sidebar.last_preview_fetch = Some(Instant::now());
    sidebar.render_stage();
    sidebar.render_inspector();
}
async fn refresh(sidebar: &mut Sidebar, client: &Client, force_preview: bool) {
    match client.snapshot().await {
        Ok(snapshot) => {
            sidebar.set_snapshot(snapshot);
            let should_preview = force_preview
                || sidebar.target_pane() != sidebar.preview_pane_id
                || sidebar
                    .last_preview_fetch
                    .is_none_or(|last| last.elapsed() > Duration::from_millis(800));
            sidebar.render_all();
            if should_preview {
                preview(sidebar, client).await;
            }
        }
        Err(error) => {
            sidebar.connected = false;
            sidebar.last_error = format!("{error:#}");
            sidebar.render_all();
        }
    }
}
async fn apply(sidebar: &mut Sidebar, client: &Client, effect: Effect) {
    match effect {
        Effect::Preview => preview(sidebar, client).await,
        Effect::Refresh => {
            refresh(sidebar, client, true).await;
            sidebar.toast(
                "success",
                "Herdr Synced",
                "Refreshed live session snapshot over herdr.sock",
            );
        }
        Effect::Create => {
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let label = format!("husk-{}", seconds % 1000);
            if client.create_workspace(&label).await.is_ok() {
                refresh(sidebar, client, true).await;
                sidebar.toast(
                    "success",
                    "Workspace Created",
                    format!("Created space '{label}' in Herdr"),
                );
            }
        }
        Effect::FocusTab(tab) => {
            let _ = client.focus_tab(&tab).await;
            refresh(sidebar, client, true).await;
        }
        Effect::Focus(item) => {
            let result = match item.kind.as_str() {
                "workspace" => client.focus_workspace(&item.workspace_id).await,
                "tab" => client.focus_tab(&item.tab_id).await,
                "pane" => client.focus_pane(&item.pane_id).await,
                "agent" => client.focus_agent(&item.pane_id).await,
                _ => return,
            };
            match result {
                Ok(()) => {
                    sidebar.toast(
                        "success",
                        "Herdr Focus Updated",
                        format!(
                            "Focused {} ({}) in live Herdr server",
                            item.label, item.kind
                        ),
                    );
                    refresh(sidebar, client, true).await;
                }
                Err(error) => sidebar.toast("error", "Herdr RPC Error", error.to_string()),
            }
        }
    }
}
