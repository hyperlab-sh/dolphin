use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RailItemSpec {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub plugin_id: String,
    pub icon: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub badge: String,
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub shortcut: String,
    pub placement: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotMeta {
    pub id: String,
    pub number: i32,
    pub title: String,
    pub bb_hotspot: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotSize {
    pub width: i32,
    pub height: i32,
    pub focused: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandSpec {
    pub id: String,
    pub title: String,
    pub category: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub shortcut: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindSpec {
    pub key: String,
    pub command_id: String,
    pub description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub when_slot: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RowHitTarget {
    pub line: i32,
    pub id: String,
    pub kind: String,
    pub action: String,
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "null_default"
    )]
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceContextPayload {
    pub workspace_id: String,
    pub workspace_label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tab_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub pane_id: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub repo_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RailActivatedPayload {
    pub rail_id: String,
    pub placement: String,
    pub active: bool,
    pub width: i32,
    pub height: i32,
    pub context: WorkspaceContextPayload,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hello: Option<HelloPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welcome: Option<WelcomePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_update: Option<SlotUpdatePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_resize: Option<SlotResizePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_key: Option<SlotKeyPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_state: Option<KeyStatePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_mouse: Option<SlotMousePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_invoke: Option<CommandInvokePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_changed: Option<ThemeChangedPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_context: Option<WorkspaceContextPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rail_activated: Option<RailActivatedPayload>,
    #[serde(skip_serializing_if = "is_false")]
    pub close_modal: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<EventPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toast: Option<ToastPayload>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HelloPayload {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub language: String,
    pub pid: i32,
    pub description: String,
    #[serde(deserialize_with = "null_default")]
    pub slots: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "null_default"
    )]
    pub rail_items: Vec<RailItemSpec>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "null_default"
    )]
    pub commands: Vec<CommandSpec>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "null_default"
    )]
    pub keybindings: Vec<KeybindSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModifierState {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    #[serde(rename = "super")]
    pub super_key: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyStatePayload {
    pub phase: String,
    pub key: String,
    pub held: bool,
    pub modifiers: ModifierState,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "null_default"
    )]
    pub held_keys: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub slot: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WelcomePayload {
    pub host_version: String,
    pub palette: Palette,
    #[serde(deserialize_with = "null_default")]
    pub slot_sizes: BTreeMap<String, SlotSize>,
    pub focused_slot: String,
    pub context: WorkspaceContextPayload,
    pub modifiers: ModifierState,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotUpdatePayload {
    pub plugin_id: String,
    pub slot: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub badge: String,
    #[serde(deserialize_with = "null_default")]
    pub lines: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "null_default"
    )]
    pub hit_targets: Vec<RowHitTarget>,
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "null_default"
    )]
    pub status_meta: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotResizePayload {
    #[serde(deserialize_with = "null_default")]
    pub slot_sizes: BTreeMap<String, SlotSize>,
    pub focused_slot: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotKeyPayload {
    pub slot: String,
    pub key: String,
    pub modifiers: ModifierState,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotMousePayload {
    pub slot: String,
    pub x: i32,
    pub y: i32,
    pub button: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hit_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hit_kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hit_action: String,
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "null_default"
    )]
    pub hit_payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandInvokePayload {
    pub command_id: String,
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "null_default"
    )]
    pub args: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeChangedPayload {
    pub palette: Palette,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventPayload {
    pub plugin_id: String,
    pub topic: String,
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "null_default"
    )]
    pub data: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToastPayload {
    pub plugin_id: String,
    pub level: String,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub name: String,
    pub canvas: String,
    pub surface: String,
    pub surface_elevated: String,
    pub surface_selected: String,
    pub surface_hover: String,
    pub border: String,
    pub border_subtle: String,
    pub border_focused: String,
    pub ink: String,
    pub ink_muted: String,
    pub ink_subtle: String,
    pub accent: String,
    pub accent_secondary: String,
    pub accent_soft: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
    pub info: String,
    pub space_green: String,
    pub space_cyan: String,
    pub space_indigo: String,
    pub space_yellow: String,
    pub space_lime: String,
    pub space_purple: String,
    pub space_pink: String,
    pub space_orange: String,
}

fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

fn is_false(v: &bool) -> bool {
    !v
}
