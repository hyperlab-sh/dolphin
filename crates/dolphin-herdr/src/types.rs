use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorktreeMetadata {
    pub checkout_path: String,
    pub is_linked_worktree: bool,
    pub repo_key: String,
    pub repo_name: String,
    pub repo_root: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub number: i32,
    pub label: String,
    pub focused: bool,
    pub pane_count: i32,
    pub tab_count: i32,
    pub active_tab_id: String,
    pub agent_status: String,
    #[serde(deserialize_with = "null_default")]
    pub tokens: BTreeMap<String, String>,
    pub worktree: Option<WorktreeMetadata>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabInfo {
    pub tab_id: String,
    pub workspace_id: String,
    pub number: i32,
    pub label: String,
    pub focused: bool,
    pub pane_count: i32,
    pub agent_status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentSession {
    pub source: String,
    pub agent: String,
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneInfo {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub terminal_id: serde_json::Value,
    pub focused: bool,
    pub cwd: String,
    pub foreground_cwd: String,
    pub agent: String,
    pub display_agent: String,
    pub terminal_title: String,
    pub terminal_title_stripped: String,
    pub agent_status: String,
    #[serde(deserialize_with = "null_default")]
    pub tokens: BTreeMap<String, String>,
    pub agent_session: Option<AgentSession>,
    pub revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentInfo {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub terminal_id: serde_json::Value,
    pub agent: String,
    pub display_agent: String,
    pub name: String,
    pub agent_status: String,
    pub focused: bool,
    pub title: String,
    pub terminal_title: String,
    pub terminal_title_stripped: String,
    pub cwd: String,
    pub foreground_cwd: String,
    #[serde(deserialize_with = "null_default")]
    pub tokens: BTreeMap<String, String>,
    pub agent_session: Option<AgentSession>,
    pub state_change_seq: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionSnapshot {
    pub version: String,
    pub protocol: i32,
    pub session_name: String,
    pub focused_workspace_id: String,
    pub focused_tab_id: String,
    pub focused_pane_id: String,
    #[serde(deserialize_with = "null_default")]
    pub workspaces: Vec<WorkspaceInfo>,
    #[serde(deserialize_with = "null_default")]
    pub tabs: Vec<TabInfo>,
    #[serde(deserialize_with = "null_default")]
    pub panes: Vec<PaneInfo>,
    #[serde(deserialize_with = "null_default")]
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneLayoutRect {
    pub pane_id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub focused: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneLayoutResult {
    pub workspace_id: String,
    pub tab_id: String,
    pub layout_mode: String,
    pub area: PaneLayoutRect,
    #[serde(deserialize_with = "null_default")]
    pub panes: Vec<PaneLayoutRect>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubscriptionEvent {
    pub seq: u64,
    pub timestamp_ms: i64,
    pub event: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
    pub agent_status: String,
    #[serde(deserialize_with = "null_default")]
    pub data: BTreeMap<String, serde_json::Value>,
}

fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}
