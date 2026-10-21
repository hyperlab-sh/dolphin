//! Husk's newline-delimited JSON plugin protocol and transport.
//! This crate deliberately has no knowledge of Herdr.
pub mod protocol;
use anyhow::{bail, Result};
pub use protocol::*;
use std::{collections::BTreeMap, env};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub const ACTIVITY_RAIL: &str = "activity-rail";
pub const SIDEBAR_NAVIGATION: &str = "sidebar-navigation";
pub const SIDEBAR_LIST: &str = "sidebar-list";
pub const THREAD_ROW_STATUS: &str = "thread-row-status";
pub const SIDEBAR_FOOTER: &str = "sidebar-footer";
pub const MAIN_HEADER: &str = "main-header";
pub const MAIN_STAGE: &str = "main-stage";
pub const SIDE_PANEL: &str = "side-panel";
pub const APP_OVERLAY: &str = "app-overlay";
pub const MODAL_SURFACE: &str = "modal-surface";
pub const FULL_CANVAS: &str = "full-canvas";
pub const WORKSPACE_STAGE: &str = "workspace-stage";
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

pub type Reader = Box<dyn AsyncBufRead + Unpin + Send>;
pub type Writer = Box<dyn AsyncWrite + Unpin + Send>;

/// Connect to the host socket, or use stdin/stdout when no path is configured.
pub async fn connect() -> Result<(Reader, Writer)> {
    if let Some(path) = env::var_os("HUSK_SOCKET_PATH").filter(|p| !p.is_empty()) {
        let stream =
            tokio::time::timeout(std::time::Duration::from_secs(3), UnixStream::connect(path))
                .await??;
        let (read, write) = stream.into_split();
        Ok((Box::new(BufReader::new(read)), Box::new(write)))
    } else {
        Ok((
            Box::new(BufReader::new(tokio::io::stdin())),
            Box::new(tokio::io::stdout()),
        ))
    }
}

/// A bounded line reader shared with JSON-RPC transports. EOF is distinct from a blank line.
pub async fn read_line<R: AsyncBufRead + Unpin + ?Sized>(
    reader: &mut R,
    max: usize,
) -> Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let count = reader
        .take((max + 1) as u64)
        .read_until(b'\n', &mut line)
        .await?;
    if count > max {
        bail!("protocol line exceeds {max} bytes");
    }
    Ok((count != 0).then_some(line))
}

pub async fn send<W: AsyncWrite + Unpin + ?Sized>(
    writer: &mut W,
    envelope: &Envelope,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(envelope)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Malformed messages are ignored, as in Husk's Go ReadLoop. Transport errors propagate.
pub async fn read_loop<R, F>(reader: &mut R, mut on_message: F) -> Result<()>
where
    R: AsyncBufRead + Unpin + ?Sized,
    F: FnMut(Envelope),
{
    while let Some(line) = read_line(reader, MAX_LINE_BYTES).await? {
        if let Ok(envelope) = serde_json::from_slice(&line) {
            on_message(envelope);
        }
    }
    Ok(())
}

impl Envelope {
    pub fn hello(payload: HelloPayload) -> Self {
        Self {
            kind: "hello".into(),
            hello: Some(payload),
            ..Self::default()
        }
    }
    pub fn slot_update(payload: SlotUpdatePayload) -> Self {
        Self {
            kind: "slot_update".into(),
            slot_update: Some(payload),
            ..Self::default()
        }
    }
    pub fn toast(payload: ToastPayload) -> Self {
        Self {
            kind: "toast".into(),
            toast: Some(payload),
            ..Self::default()
        }
    }
    pub fn workspace_context(payload: WorkspaceContextPayload) -> Self {
        Self {
            kind: "workspace_context".into(),
            workspace_context: Some(payload),
            ..Self::default()
        }
    }
}

impl Palette {
    pub fn space_color(&self, key: &str) -> &str {
        match key {
            "space_green" => &self.space_green,
            "space_cyan" => &self.space_cyan,
            "space_indigo" => &self.space_indigo,
            "space_yellow" => &self.space_yellow,
            "space_lime" => &self.space_lime,
            "space_purple" => &self.space_purple,
            "space_pink" => &self.space_pink,
            "space_orange" => &self.space_orange,
            _ => &self.accent,
        }
    }
    pub fn tokyo_night() -> Self {
        // The same initial palette used by the spike; welcome/theme_changed replace it.
        serde_json::from_value(serde_json::json!({
            "name":"Tokyo Night", "canvas":"#16161e", "surface":"#1a1b26",
            "surface_elevated":"#24283b", "surface_selected":"#283457", "surface_hover":"#1f2335",
            "border":"#3b4261", "border_subtle":"#292e42", "border_focused":"#7aa2f7",
            "ink":"#c0caf5", "ink_muted":"#a9b1d6", "ink_subtle":"#565f89",
            "accent":"#7aa2f7", "accent_secondary":"#bb9af7", "accent_soft":"#3d59a1",
            "success":"#9ece6a", "warning":"#e0af68", "danger":"#f7768e", "info":"#7dcfff",
            "space_green":"#9ece6a", "space_cyan":"#7dcfff", "space_indigo":"#7aa2f7",
            "space_yellow":"#e0af68", "space_lime":"#73daca", "space_purple":"#bb9af7",
            "space_pink":"#f7768e", "space_orange":"#ff9e64"
        }))
        .expect("static palette")
    }
}

pub fn sizes(entries: &[(&str, i32, i32)]) -> BTreeMap<String, SlotSize> {
    entries
        .iter()
        .map(|(slot, width, height)| {
            (
                (*slot).into(),
                SlotSize {
                    width: *width,
                    height: *height,
                    visible: true,
                    focused: *slot == SIDEBAR_LIST,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_go_payload_round_trips() {
        let fixtures: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/fixtures/envelopes.json")).unwrap();
        assert_eq!(fixtures.len(), 14);
        for fixture in fixtures {
            let env: Envelope = serde_json::from_value(fixture.clone()).unwrap();
            assert_eq!(serde_json::to_value(env).unwrap(), fixture);
        }
    }
    #[test]
    fn omitted_empty_and_null_collections_follow_go_decoding_and_omitempty() {
        use serde_json::{json, Value};
        for mode in ["omitted", "empty", "null"] {
            let mut hello = json!({"type":"hello","hello":{}});
            let mut update = json!({"type":"slot_update","slot_update":{}});
            let mut welcome = json!({"type":"welcome","welcome":{}});
            if mode != "omitted" {
                let array = if mode == "null" {
                    Value::Null
                } else {
                    json!([])
                };
                let map = if mode == "null" {
                    Value::Null
                } else {
                    json!({})
                };
                for field in ["slots", "commands", "keybindings", "rail_items"] {
                    hello["hello"][field] = array.clone();
                }
                update["slot_update"]["lines"] = array.clone();
                update["slot_update"]["hit_targets"] = array;
                update["slot_update"]["status_meta"] = map.clone();
                welcome["welcome"]["slot_sizes"] = map;
            }
            let hello: Envelope = serde_json::from_value(hello).unwrap();
            assert!(hello.hello.as_ref().unwrap().slots.is_empty());
            let encoded = serde_json::to_value(hello).unwrap();
            assert_eq!(encoded["hello"]["slots"], json!([]));
            for field in ["commands", "keybindings", "rail_items"] {
                assert!(encoded["hello"].get(field).is_none(), "{mode}: {field}");
            }
            let update: Envelope = serde_json::from_value(update).unwrap();
            let encoded = serde_json::to_value(update).unwrap();
            assert_eq!(encoded["slot_update"]["lines"], json!([]));
            for field in ["hit_targets", "status_meta"] {
                assert!(
                    encoded["slot_update"].get(field).is_none(),
                    "{mode}: {field}"
                );
            }
            let welcome: Envelope = serde_json::from_value(welcome).unwrap();
            assert!(welcome.welcome.unwrap().slot_sizes.is_empty());
        }
        let envelope: Envelope =
            serde_json::from_value(json!({"type":"close_modal","close_modal":false,"hello":null}))
                .unwrap();
        assert_eq!(
            serde_json::to_value(envelope).unwrap(),
            json!({"type":"close_modal"})
        );
    }
    #[tokio::test]
    async fn framing_skips_bad_json_and_accepts_final_line() {
        let data = b"\nnot json\n{\"type\":\"close_modal\",\"close_modal\":true}";
        let mut reader = BufReader::new(&data[..]);
        let mut seen = Vec::new();
        read_loop(&mut reader, |e| seen.push(e)).await.unwrap();
        assert_eq!(seen.len(), 1);
        assert!(seen[0].close_modal);
    }
    #[tokio::test]
    async fn send_adds_one_newline_and_limit_is_enforced() {
        let mut bytes = Vec::new();
        send(
            &mut bytes,
            &Envelope {
                kind: "close_modal".into(),
                close_modal: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(bytes, b"{\"type\":\"close_modal\",\"close_modal\":true}\n");
        assert!(read_line(&mut &bytes[..], 4).await.is_err());
    }
}
