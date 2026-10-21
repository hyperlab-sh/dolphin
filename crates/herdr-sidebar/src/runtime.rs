//! Single-owner sidebar actor. Socket/RPC tasks return data, never mutate UI state.
use crate::{hello, Effect, Sidebar, FRAMES};
use anyhow::{anyhow, Result};
use dolphin_herdr::{Client, PaneLayoutResult, SessionSnapshot, SubscriptionEvent};
use husk_plugin::{read_loop, send, Envelope, Reader, Writer};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::{
    sync::mpsc,
    task::{AbortHandle, JoinSet},
};

#[derive(Debug)]
enum Connection {
    Connected,
    Disconnected(String),
    Event(SubscriptionEvent),
}

enum Completion {
    Snapshot {
        generation: u64,
        result: Result<SessionSnapshot>,
    },
    Preview {
        generation: u64,
        pane: String,
        result: Result<PaneLayoutResult>,
    },
    Mutation {
        refresh: bool,
        toast: Option<(&'static str, &'static str, String)>,
    },
}

#[derive(Default)]
struct Jobs {
    tasks: JoinSet<Completion>,
    snapshot: Option<AbortHandle>,
    preview: Option<AbortHandle>,
    snapshot_generation: u64,
    preview_generation: u64,
    force_preview: bool,
    refresh_toast: bool,
    resnapshot: bool,
}
impl Jobs {
    fn cancel_preview(&mut self) {
        self.preview_generation += 1;
        if let Some(task) = self.preview.take() {
            task.abort();
        }
    }
    fn invalidate_connection(&mut self) {
        self.snapshot_generation += 1;
        if let Some(task) = self.snapshot.take() {
            task.abort();
        }
        self.cancel_preview();
        self.resnapshot = false;
    }
}

struct Controller {
    sidebar: Sidebar,
    client: Client,
    jobs: Jobs,
    stream_healthy: bool,
    last_event_snapshot: Instant,
}
impl Controller {
    fn new(sidebar: Sidebar, client: Client) -> Self {
        Self {
            sidebar,
            client,
            jobs: Jobs::default(),
            stream_healthy: false,
            last_event_snapshot: Instant::now() - Duration::from_secs(1),
        }
    }
    fn snapshot(&mut self, force_preview: bool, toast: bool) {
        self.jobs.force_preview |= force_preview;
        self.jobs.refresh_toast |= toast;
        if self.jobs.snapshot.is_some() {
            // Coalesce bursts without starving a slow snapshot by repeatedly cancelling it.
            self.jobs.resnapshot = true;
            return;
        }
        self.jobs.snapshot_generation += 1;
        let generation = self.jobs.snapshot_generation;
        let client = self.client.clone();
        self.jobs.snapshot = Some(self.jobs.tasks.spawn(async move {
            Completion::Snapshot {
                generation,
                result: client.snapshot().await,
            }
        }));
    }
    fn preview(&mut self) {
        self.jobs.cancel_preview();
        let pane = self.sidebar.target_pane();
        if self.sidebar.preview_pane_id != pane {
            self.sidebar.preview_layout = None;
            self.sidebar.preview_pane_id.clear();
            self.sidebar.render_stage();
        }
        if pane.is_empty() {
            return;
        }
        let generation = self.jobs.preview_generation;
        let client = self.client.clone();
        self.jobs.preview = Some(self.jobs.tasks.spawn(async move {
            Completion::Preview {
                generation,
                result: client.pane_layout(&pane).await,
                pane,
            }
        }));
    }
    fn host(&mut self, envelope: Envelope) {
        let before = self.sidebar.target_pane();
        let effects = self.sidebar.handle_host(envelope);
        let requested_preview = effects.contains(&Effect::Preview);
        for effect in effects {
            match effect {
                Effect::Preview => self.preview(),
                Effect::Refresh => self.snapshot(true, true),
                effect => {
                    let client = self.client.clone();
                    self.jobs.tasks.spawn(mutate(client, effect));
                }
            }
        }
        if !requested_preview && before != self.sidebar.target_pane() {
            self.preview();
        }
    }
    fn connection(&mut self, connection: Connection) {
        match connection {
            Connection::Connected => {
                // Never trust a pre-subscription snapshot: state may have changed in the gap.
                self.jobs.invalidate_connection();
                self.stream_healthy = true;
                self.sidebar.connected = false;
                self.sidebar.last_error = "Synchronizing Herdr after subscription".into();
                self.sidebar.render_all();
                self.snapshot(true, false);
            }
            Connection::Disconnected(error) => {
                self.jobs.invalidate_connection();
                self.stream_healthy = false;
                self.sidebar.connected = false;
                self.sidebar.last_error = error;
                self.sidebar.render_all();
            }
            Connection::Event(event) => {
                if event.pane_id == self.sidebar.self_pane_id
                    && !event.pane_id.is_empty()
                    && event.event == "pane.updated"
                {
                    return;
                }
                if self.last_event_snapshot.elapsed() < Duration::from_millis(150) {
                    return;
                }
                self.last_event_snapshot = Instant::now();
                self.sidebar.event_count += 1;
                self.sidebar.last_event = event.event;
                self.snapshot(false, false);
            }
        }
    }
    fn complete(&mut self, completion: Completion) {
        match completion {
            Completion::Snapshot { generation, result }
                if generation == self.jobs.snapshot_generation =>
            {
                self.jobs.snapshot = None;
                let force = std::mem::take(&mut self.jobs.force_preview);
                match result {
                    Ok(snapshot) => {
                        self.sidebar.set_snapshot(snapshot);
                        // A successful manual RPC must not hide a broken event connection.
                        self.sidebar.connected = self.stream_healthy;
                        if !self.stream_healthy {
                            self.sidebar.last_error =
                                "Herdr event subscription is disconnected".into();
                        }
                        let preview = force
                            || self.sidebar.target_pane() != self.sidebar.preview_pane_id
                            || self
                                .sidebar
                                .last_preview_fetch
                                .is_none_or(|last| last.elapsed() > Duration::from_millis(800));
                        self.sidebar.render_all();
                        if preview {
                            self.preview();
                        }
                    }
                    Err(error) => {
                        self.jobs.cancel_preview();
                        self.sidebar.connected = false;
                        self.sidebar.last_error = format!("{error:#}");
                        self.sidebar.render_all();
                    }
                }
                if std::mem::take(&mut self.jobs.refresh_toast) {
                    self.sidebar.toast(
                        "success",
                        "Herdr Synced",
                        "Refreshed live session snapshot over herdr.sock",
                    );
                }
                if std::mem::take(&mut self.jobs.resnapshot) {
                    self.snapshot(false, false);
                }
            }
            Completion::Preview {
                generation,
                pane,
                result,
            } if generation == self.jobs.preview_generation
                && pane == self.sidebar.target_pane() =>
            {
                self.jobs.preview = None;
                self.sidebar.preview_layout = result.ok();
                self.sidebar.preview_pane_id = pane;
                self.sidebar.last_preview_fetch = Some(Instant::now());
                self.sidebar.render_stage();
                self.sidebar.render_inspector();
            }
            Completion::Mutation { refresh, toast } => {
                if let Some((level, title, message)) = toast {
                    self.sidebar.toast(level, title, message);
                }
                if refresh {
                    self.snapshot(true, false);
                }
            }
            // Cancelled/superseded snapshots and layouts cannot overwrite newer state.
            _ => {}
        }
    }
}

async fn mutate(client: Client, effect: Effect) -> Completion {
    let (refresh, toast) = match effect {
        Effect::Create => {
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let label = format!("husk-{}", seconds % 1000);
            if client.create_workspace(&label).await.is_ok() {
                (
                    true,
                    Some((
                        "success",
                        "Workspace Created",
                        format!("Created space '{label}' in Herdr"),
                    )),
                )
            } else {
                (false, None)
            }
        }
        Effect::FocusTab(tab) => {
            let _ = client.focus_tab(&tab).await;
            (true, None)
        }
        Effect::Focus(item) => {
            let result = match item.kind.as_str() {
                "workspace" => client.focus_workspace(&item.workspace_id).await,
                "tab" => client.focus_tab(&item.tab_id).await,
                "pane" => client.focus_pane(&item.pane_id).await,
                "agent" => client.focus_agent(&item.pane_id).await,
                _ => {
                    return Completion::Mutation {
                        refresh: false,
                        toast: None,
                    }
                }
            };
            match result {
                Ok(()) => (
                    true,
                    Some((
                        "success",
                        "Herdr Focus Updated",
                        format!(
                            "Focused {} ({}) in live Herdr server",
                            item.label, item.kind
                        ),
                    )),
                ),
                Err(error) => (false, Some(("error", "Herdr RPC Error", error.to_string()))),
            }
        }
        _ => (false, None),
    };
    Completion::Mutation { refresh, toast }
}

async fn subscriptions(client: Client, sender: mpsc::Sender<Connection>, retry: Duration) {
    loop {
        let error = match client.subscribe_events().await {
            Ok(mut stream) => {
                if sender.send(Connection::Connected).await.is_err() {
                    return;
                }
                loop {
                    match stream.next_event().await {
                        Ok(Some(event)) => {
                            if sender.send(Connection::Event(event)).await.is_err() {
                                return;
                            }
                        }
                        Ok(None) => break "Herdr event subscription closed".into(),
                        Err(error) => break format!("Herdr event subscription failed: {error:#}"),
                    }
                }
            }
            Err(error) => format!("Herdr event subscription failed: {error:#}"),
        };
        if sender.send(Connection::Disconnected(error)).await.is_err() {
            return;
        }
        tokio::time::sleep(retry).await;
    }
}

/// Runs over an injected Husk transport; mock duplex streams need no live application.
/// Every background task/socket is owned here and cancelled on host EOF, error, or cancellation.
pub async fn run(sidebar: Sidebar, client: Client, reader: Reader, writer: Writer) -> Result<()> {
    run_inner(sidebar, client, reader, writer, Duration::from_millis(1500)).await
}
async fn run_inner(
    sidebar: Sidebar,
    client: Client,
    mut reader: Reader,
    mut writer: Writer,
    retry: Duration,
) -> Result<()> {
    send(&mut writer, &hello()).await?;
    let mut controller = Controller::new(sidebar, client);
    let (host_tx, mut host_rx) = mpsc::unbounded_channel();
    let (connection_tx, mut connection_rx) = mpsc::channel(32);
    let mut services = JoinSet::new();
    services.spawn(async move {
        let result = read_loop(&mut reader, |env| {
            let _ = host_tx.send(Ok(env));
        })
        .await;
        if let Err(error) = result {
            let _ = host_tx.send(Err(error));
        }
    });
    services.spawn(subscriptions(
        controller.client.clone(),
        connection_tx,
        retry,
    ));
    controller.snapshot(true, false);
    controller.sidebar.last_error = "Connecting to Herdr".into();
    controller.sidebar.render_all();
    let mut ticker = tokio::time::interval(Duration::from_millis(220));
    let result = async {
        loop {
            // RPCs were only scheduled above/below: queued UI updates never wait for them.
            for envelope in controller.sidebar.outbox.drain(..) { send(&mut writer, &envelope).await?; }
            tokio::select! {
                host = host_rx.recv() => {
                    let Some(host) = host else { break; };
                    controller.host(host?);
                }
                connection = connection_rx.recv() => {
                    let Some(connection) = connection else { return Err(anyhow!("Herdr subscription task stopped")); };
                    controller.connection(connection);
                }
                completion = controller.jobs.tasks.join_next(), if !controller.jobs.tasks.is_empty() => {
                    match completion {
                        Some(Ok(completion)) => controller.complete(completion),
                        Some(Err(error)) if error.is_cancelled() => {},
                        Some(Err(error)) => return Err(error.into()),
                        None => {},
                    }
                }
                _ = ticker.tick() => {
                    controller.sidebar.spinner_frame = (controller.sidebar.spinner_frame + 1) % FRAMES.len();
                    if controller.sidebar.snapshot.as_ref().is_some_and(|s| s.agents.iter().any(|a| a.agent_status == "working")) { controller.sidebar.render_list(); }
                }
            }
        }
        Ok(())
    }.await;
    services.abort_all();
    controller.jobs.tasks.abort_all();
    while services.join_next().await.is_some() {}
    while controller.jobs.tasks.join_next().await.is_some() {}
    result
}

#[cfg(test)]
mod tests;
