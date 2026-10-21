//! Herdr-specific JSON-RPC client. Each call and event subscription owns its socket.
mod types;
use anyhow::{anyhow, bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::{
    env,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
pub use types::*;

pub const EVENT_TYPES: &[&str] = &[
    "workspace.created",
    "workspace.updated",
    "workspace.renamed",
    "workspace.closed",
    "workspace.focused",
    "tab.created",
    "tab.closed",
    "tab.focused",
    "tab.renamed",
    "pane.created",
    "pane.closed",
    "pane.focused",
    "pane.moved",
    "pane.exited",
    "pane.agent_detected",
];
const MAX_LINE: usize = 2 * 1024 * 1024;

pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}
pub fn resolve_socket_path() -> PathBuf {
    socket_path_from(
        env::var_os("HERDR_SOCKET_PATH").map(PathBuf::from),
        env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        &home_dir(),
    )
}
fn socket_path_from(explicit: Option<PathBuf>, config: Option<PathBuf>, home: &Path) -> PathBuf {
    explicit
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            config
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| home.join(".config"))
                .join("herdr/herdr.sock")
        })
}
pub fn clean_invisible(text: &str) -> String {
    text.chars()
        .filter(|c| !('\u{2060}'..='\u{206f}').contains(c))
        .collect()
}
impl WorkspaceInfo {
    pub fn space_color_key(&self) -> &str {
        self.tokens
            .keys()
            .find(|k| k.starts_with("space_"))
            .map(String::as_str)
            .unwrap_or("space_indigo")
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    pub socket_path: PathBuf,
    sequence: Arc<AtomicU64>,
}
#[derive(Deserialize)]
struct RpcError {
    code: Value,
    message: String,
}
#[derive(Deserialize)]
struct Response {
    id: String,
    #[serde(default)]
    result: Value,
    error: Option<RpcError>,
}
impl Response {
    fn result(self, expected_id: &str) -> Result<Value> {
        if self.id != expected_id {
            bail!("Herdr response ID mismatch");
        }
        if let Some(error) = self.error {
            bail!("{}: {}", error.code, error.message);
        }
        Ok(self.result)
    }
}
async fn line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    let n = reader
        .take((MAX_LINE + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await?;
    if n > MAX_LINE {
        bail!("Herdr response exceeds {MAX_LINE} bytes");
    }
    Ok((n > 0).then_some(bytes))
}
impl Default for Client {
    fn default() -> Self {
        Self::new(resolve_socket_path())
    }
}
impl Client {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            sequence: Arc::new(AtomicU64::new(0)),
        }
    }
    fn id(&self) -> String {
        format!(
            "husk-herdr-{}-{}",
            std::process::id(),
            self.sequence.fetch_add(1, Ordering::Relaxed)
        )
    }
    pub async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        tokio::time::timeout(Duration::from_secs(3), async {
            let id = self.id();
            let mut conn = UnixStream::connect(&self.socket_path)
                .await
                .context("connect to Herdr")?;
            let mut request =
                serde_json::to_vec(&json!({"id":id,"method":method,"params":params}))?;
            request.push(b'\n');
            conn.write_all(&request).await?;
            let bytes = line(&mut BufReader::new(conn))
                .await?
                .ok_or_else(|| anyhow!("Herdr closed without a response"))?;
            let result = serde_json::from_slice::<Response>(&bytes)?.result(&id)?;
            Ok(serde_json::from_value(result)?)
        })
        .await
        .context("Herdr RPC timed out")?
    }
    pub async fn snapshot(&self) -> Result<SessionSnapshot> {
        #[derive(Deserialize)]
        struct Wrapper {
            snapshot: SessionSnapshot,
        }
        let mut snap = self
            .call::<Wrapper>("session.snapshot", json!({}))
            .await?
            .snapshot;
        if snap.session_name.is_empty() {
            snap.session_name = "herdr".into();
        }
        for ws in &mut snap.workspaces {
            ws.label = clean_invisible(&ws.label);
            ws.tokens.values_mut().for_each(|v| *v = clean_invisible(v));
        }
        for tab in &mut snap.tabs {
            tab.label = clean_invisible(&tab.label);
        }
        for pane in &mut snap.panes {
            pane.display_agent = clean_invisible(&pane.display_agent);
            pane.terminal_title = clean_invisible(&pane.terminal_title);
            pane.terminal_title_stripped = clean_invisible(&pane.terminal_title_stripped);
            pane.tokens
                .values_mut()
                .for_each(|v| *v = clean_invisible(v));
        }
        for agent in &mut snap.agents {
            agent.display_agent = clean_invisible(&agent.display_agent);
            agent.title = clean_invisible(&agent.title);
            agent.terminal_title = clean_invisible(&agent.terminal_title);
            agent.terminal_title_stripped = clean_invisible(&agent.terminal_title_stripped);
            agent
                .tokens
                .values_mut()
                .for_each(|v| *v = clean_invisible(v));
        }
        Ok(snap)
    }
    pub async fn focus_workspace(&self, id: &str) -> Result<()> {
        self.call::<Value>("workspace.focus", json!({"workspace_id":id}))
            .await?;
        Ok(())
    }
    pub async fn create_workspace(&self, label: &str) -> Result<()> {
        self.call::<Value>(
            "workspace.create",
            if label.is_empty() {
                json!({})
            } else {
                json!({"label":label})
            },
        )
        .await?;
        Ok(())
    }
    pub async fn focus_tab(&self, id: &str) -> Result<()> {
        self.call::<Value>("tab.focus", json!({"tab_id":id}))
            .await?;
        Ok(())
    }
    pub async fn focus_pane(&self, id: &str) -> Result<()> {
        self.call::<Value>("pane.focus", json!({"pane_id":id}))
            .await?;
        Ok(())
    }
    pub async fn focus_agent(&self, id: &str) -> Result<()> {
        self.call::<Value>("agent.focus", json!({"pane_id":id}))
            .await?;
        Ok(())
    }
    pub async fn read_pane_visible(&self, id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Read {
            text: String,
        }
        #[derive(Deserialize)]
        struct Wrapper {
            read: Read,
        }
        Ok(self
            .call::<Wrapper>(
                "pane.read",
                json!({"pane_id":id,"source":"visible","format":"text","strip_ansi":true}),
            )
            .await?
            .read
            .text)
    }
    pub async fn pane_layout(&self, id: &str) -> Result<PaneLayoutResult> {
        let value = self
            .call::<Value>("pane.layout", json!({"pane_id":id}))
            .await?;
        decode_layout(value)
    }
    /// Dropping this stream closes only the dedicated subscription connection.
    pub async fn subscribe_events(&self) -> Result<EventStream> {
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut conn = UnixStream::connect(&self.socket_path).await?;
            let id = self.id();
            let subscriptions: Vec<_> = EVENT_TYPES.iter().map(|kind| json!({"type":kind})).collect();
            let mut bytes = serde_json::to_vec(&json!({"id":id,"method":"events.subscribe","params":{"subscriptions":subscriptions}}))?;
            bytes.push(b'\n'); conn.write_all(&bytes).await?;
            let mut reader = BufReader::new(conn);
            let ack = line(&mut reader).await?.ok_or_else(|| anyhow!("missing subscription acknowledgement"))?;
            serde_json::from_slice::<Response>(&ack)?.result(&id)?;
            Ok(EventStream { reader })
        }).await.context("Herdr subscription timed out")?
    }
}
fn decode_layout(value: Value) -> Result<PaneLayoutResult> {
    #[derive(Deserialize)]
    struct Pane {
        pane_id: String,
        focused: bool,
        rect: PaneLayoutRect,
    }
    #[derive(Deserialize)]
    struct Layout {
        workspace_id: String,
        tab_id: String,
        #[serde(default)]
        layout_mode: String,
        area: PaneLayoutRect,
        panes: Vec<Pane>,
    }
    #[derive(Deserialize)]
    struct Wrapper {
        layout: Layout,
    }
    let raw = serde_json::from_value::<Wrapper>(value)?.layout;
    Ok(PaneLayoutResult {
        workspace_id: raw.workspace_id,
        tab_id: raw.tab_id,
        layout_mode: raw.layout_mode,
        area: raw.area,
        panes: raw
            .panes
            .into_iter()
            .map(|p| PaneLayoutRect {
                pane_id: p.pane_id,
                focused: p.focused,
                ..p.rect
            })
            .collect(),
    })
}
pub struct EventStream {
    reader: BufReader<UnixStream>,
}
impl EventStream {
    pub async fn next_event(&mut self) -> Result<Option<SubscriptionEvent>> {
        while let Some(bytes) = line(&mut self.reader).await? {
            if let Ok(event) = serde_json::from_slice::<SubscriptionEvent>(&bytes) {
                if !event.event.is_empty() {
                    return Ok(Some(event));
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn socket_precedence() {
        let home = Path::new("/home/example");
        assert_eq!(
            socket_path_from(
                Some("/tmp/custom.sock".into()),
                Some("/tmp/config".into()),
                home
            ),
            PathBuf::from("/tmp/custom.sock")
        );
        assert_eq!(
            socket_path_from(None, Some("/tmp/config".into()), home),
            PathBuf::from("/tmp/config/herdr/herdr.sock")
        );
        assert_eq!(
            socket_path_from(Some("".into()), None, home),
            home.join(".config/herdr/herdr.sock")
        );
    }
    #[test]
    fn invisible_markers_are_removed() {
        assert_eq!(clean_invisible("ab\u{2060}cd\u{206f}"), "abcd");
    }
    #[test]
    fn recorded_cli_responses_decode() {
        let ws: Value =
            serde_json::from_str(include_str!("../tests/fixtures/workspaces.json")).unwrap();
        let workspaces: Vec<WorkspaceInfo> =
            serde_json::from_value(ws["result"]["workspaces"].clone()).unwrap();
        assert!(!workspaces.is_empty());
        let ag: Value =
            serde_json::from_str(include_str!("../tests/fixtures/agents.json")).unwrap();
        let agents: Vec<AgentInfo> =
            serde_json::from_value(ag["result"]["agents"].clone()).unwrap();
        assert!(!agents.is_empty());
        let layout: Value =
            serde_json::from_str(include_str!("../tests/fixtures/layout.json")).unwrap();
        let layout = decode_layout(layout["result"].clone()).unwrap();
        assert!(!layout.panes.is_empty());
        assert!(layout.panes[0].width > 0);
    }
    async fn mock(reply: Value) -> (Client, tokio::task::JoinHandle<Value>, PathBuf) {
        mock_stream(reply, b"").await
    }
    async fn mock_stream(
        reply: Value,
        trailer: &'static [u8],
    ) -> (Client, tokio::task::JoinHandle<Value>, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = PathBuf::from("/tmp").join(format!(
            "dolphin-test-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let req: Value =
                serde_json::from_slice(&line(&mut reader).await.unwrap().unwrap()).unwrap();
            let mut reply = reply;
            reply["id"] = req["id"].clone();
            let mut bytes = serde_json::to_vec(&reply).unwrap();
            bytes.push(b'\n');
            bytes.extend_from_slice(trailer);
            reader.get_mut().write_all(&bytes).await.unwrap();
            req
        });
        (Client::new(path.clone()), task, path)
    }
    #[tokio::test]
    async fn typed_calls_send_expected_methods_and_params() {
        for (method, params) in [
            ("workspace.focus", json!({"workspace_id":"w1"})),
            ("workspace.create", json!({"label":"test"})),
            ("tab.focus", json!({"tab_id":"t1"})),
            ("pane.focus", json!({"pane_id":"p1"})),
            ("agent.focus", json!({"pane_id":"p1"})),
        ] {
            let (cli, task, path) = mock(json!({"result":{}})).await;
            match method {
                "workspace.focus" => cli.focus_workspace("w1").await,
                "workspace.create" => cli.create_workspace("test").await,
                "tab.focus" => cli.focus_tab("t1").await,
                "pane.focus" => cli.focus_pane("p1").await,
                _ => cli.focus_agent("p1").await,
            }
            .unwrap();
            let req = task.await.unwrap();
            assert_eq!(req["method"], method);
            assert_eq!(req["params"], params);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[tokio::test]
    async fn snapshot_read_layout_and_subscription_use_wire_wrappers() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/snapshot.json")).unwrap();
        let (cli, task, path) = mock(fixture).await;
        let snap = cli.snapshot().await.unwrap();
        assert!(!snap.panes.is_empty());
        assert_eq!(task.await.unwrap()["method"], "session.snapshot");
        std::fs::remove_file(path).unwrap();
        let (cli, task, path) =
            mock(json!({"result":{"type":"pane.read","read":{"text":"hello"}}})).await;
        assert_eq!(cli.read_pane_visible("p1").await.unwrap(), "hello");
        assert_eq!(
            task.await.unwrap()["params"],
            json!({"pane_id":"p1","source":"visible","format":"text","strip_ansi":true})
        );
        std::fs::remove_file(path).unwrap();
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/layout.json")).unwrap();
        let (cli, task, path) = mock(fixture).await;
        assert!(!cli.pane_layout("p1").await.unwrap().panes.is_empty());
        assert_eq!(task.await.unwrap()["method"], "pane.layout");
        std::fs::remove_file(path).unwrap();
        let (cli, task, path) = mock(json!({"result":{}})).await;
        let mut stream = cli.subscribe_events().await.unwrap();
        assert!(stream.next_event().await.unwrap().is_none());
        let req = task.await.unwrap();
        assert_eq!(req["method"], "events.subscribe");
        assert_eq!(
            req["params"]["subscriptions"],
            json!(EVENT_TYPES
                .iter()
                .map(|kind| json!({"type":kind}))
                .collect::<Vec<_>>())
        );
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn subscription_skips_noise_and_decodes_events() {
        let (cli, task, path) = mock_stream(json!({"result":{}}), b"\ninvalid\n{\"event\":\"pane.focused\",\"seq\":7,\"pane_id\":\"p1\",\"data\":{\"focused\":true}}\n").await;
        let mut events = cli.subscribe_events().await.unwrap();
        let event = events.next_event().await.unwrap().unwrap();
        assert_eq!(event.seq, 7);
        assert_eq!(event.event, "pane.focused");
        assert_eq!(event.pane_id, "p1");
        assert_eq!(event.data["focused"], true);
        assert!(events.next_event().await.unwrap().is_none());
        task.await.unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn mismatched_response_ids_are_rejected() {
        let response: Response = serde_json::from_value(json!({"id":"wrong","result":{}})).unwrap();
        assert!(response.result("expected").is_err());
    }
    #[tokio::test]
    async fn snapshot_normalizes_the_same_fields_as_the_spike() {
        let (cli, task, path) = mock(json!({"result":{"snapshot":{
            "workspaces":[{"label":"sp\u{2060}ace", "tokens":{"model":"exa\u{206f}mple"}}],
            "tabs":[{"label":"ta\u{2060}b"}],
            "panes":[{"display_agent":"cl\u{2060}i","terminal_title":"ti\u{2060}tle","terminal_title_stripped":"ti\u{2060}tle"}],
            "agents":[{"title":"age\u{2060}nt","tokens":{"model":"exa\u{206f}mple"}}]
        }}})).await;
        let snap = cli.snapshot().await.unwrap();
        assert_eq!(snap.session_name, "herdr");
        assert_eq!(snap.workspaces[0].label, "space");
        assert_eq!(snap.workspaces[0].tokens["model"], "example");
        assert_eq!(snap.tabs[0].label, "tab");
        assert_eq!(snap.panes[0].display_agent, "cli");
        assert_eq!(snap.panes[0].terminal_title, "title");
        assert_eq!(snap.panes[0].terminal_title_stripped, "title");
        assert_eq!(snap.agents[0].title, "agent");
        assert_eq!(snap.agents[0].tokens["model"], "example");
        task.await.unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn rpc_errors_propagate() {
        let (cli, task, path) =
            mock(json!({"error":{"code":"not_found","message":"No pane"}})).await;
        assert!(cli
            .focus_pane("missing")
            .await
            .unwrap_err()
            .to_string()
            .contains("No pane"));
        task.await.unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
