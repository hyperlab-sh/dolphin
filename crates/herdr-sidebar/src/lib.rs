//! Sidebar state machine and rendering, isolated from live sockets for testing.
mod render;
use dolphin_herdr::{home_dir, AgentInfo, PaneLayoutResult, SessionSnapshot};
use husk_plugin::*;
pub use render::fit_line;
use std::collections::BTreeMap;

pub const PLUGIN_ID: &str = "dolphin.herdr-sidebar";
pub const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Split,
    Tree,
    Agents,
}
impl ViewMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Split => "split",
            Self::Tree => "tree",
            Self::Agents => "agents",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Split => Self::Tree,
            Self::Tree => Self::Agents,
            Self::Agents => Self::Split,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Priority,
    Spaces,
}
impl SortMode {
    pub fn name(self) -> &'static str {
        if self == Self::Priority {
            "priority"
        } else {
            "spaces"
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Refresh,
    Preview,
    Focus(Item),
    FocusTab(String),
    Create,
}

pub struct Sidebar {
    pub palette: Palette,
    pub slot_sizes: BTreeMap<String, SlotSize>,
    pub focused_slot: String,
    pub modifiers: ModifierState,
    pub snapshot: Option<SessionSnapshot>,
    pub connected: bool,
    pub last_error: String,
    pub socket_path: String,
    pub event_count: u64,
    pub last_event: String,
    pub spinner_frame: usize,
    pub view_mode: ViewMode,
    pub sort_mode: SortMode,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub items: Vec<Item>,
    pub selected_workspace_id: String,
    pub selected_tab_id: String,
    pub selected_pane_id: String,
    pub self_pane_id: String,
    pub preview_pane_id: String,
    pub preview_layout: Option<PaneLayoutResult>,
    pub last_preview_fetch: Option<std::time::Instant>,
    pub outbox: Vec<Envelope>,
}
impl Sidebar {
    pub fn new(socket_path: String, self_pane_id: String) -> Self {
        Self {
            palette: Palette::tokyo_night(),
            slot_sizes: sizes(&[
                (SIDEBAR_NAVIGATION, 36, 2),
                (SIDEBAR_LIST, 36, 28),
                (SIDEBAR_FOOTER, 36, 2),
                (MAIN_HEADER, 62, 2),
                (MAIN_STAGE, 62, 32),
                (SIDE_PANEL, 36, 36),
            ]),
            focused_slot: SIDEBAR_LIST.into(),
            modifiers: ModifierState::default(),
            snapshot: None,
            connected: false,
            last_error: String::new(),
            socket_path,
            event_count: 0,
            last_event: String::new(),
            spinner_frame: 0,
            view_mode: ViewMode::Split,
            sort_mode: SortMode::Priority,
            cursor: 0,
            scroll_offset: 0,
            items: Vec::new(),
            selected_workspace_id: String::new(),
            selected_tab_id: String::new(),
            selected_pane_id: String::new(),
            self_pane_id,
            preview_pane_id: String::new(),
            preview_layout: None,
            last_preview_fetch: None,
            outbox: Vec::new(),
        }
    }
    pub fn set_snapshot(&mut self, snap: SessionSnapshot) {
        let initial = self.selected_workspace_id.is_empty();
        self.snapshot = Some(snap);
        self.connected = true;
        self.last_error.clear();
        self.rebuild_items();
        if initial {
            if let Some(snap) = &self.snapshot {
                self.cursor = self
                    .items
                    .iter()
                    .position(|it| {
                        it.kind == "workspace" && it.workspace_id == snap.focused_workspace_id
                    })
                    .unwrap_or(0);
            }
        }
        self.sync_selection();
    }
    pub fn sorted_agents(&self) -> Vec<AgentInfo> {
        let Some(snap) = &self.snapshot else {
            return Vec::new();
        };
        let mut agents = snap.agents.clone();
        let number = |id: &str| {
            snap.workspaces
                .iter()
                .find(|w| w.workspace_id == id)
                .map(|w| w.number)
                .unwrap_or(0)
        };
        agents.sort_by_key(|a| {
            (
                if self.sort_mode == SortMode::Priority {
                    status_rank(&a.agent_status)
                } else {
                    0
                },
                number(&a.workspace_id),
                a.pane_id.clone(),
            )
        });
        agents
    }
    pub fn rebuild_items(&mut self) {
        self.items.clear();
        let Some(snap) = &self.snapshot else { return };
        if self.view_mode != ViewMode::Agents {
            for ws in &snap.workspaces {
                self.items.push(Item {
                    id: format!("ws:{}", ws.workspace_id),
                    kind: "workspace".into(),
                    workspace_id: ws.workspace_id.clone(),
                    tab_id: ws.active_tab_id.clone(),
                    pane_id: String::new(),
                    label: ws.label.clone(),
                });
                if self.view_mode == ViewMode::Tree {
                    for tab in snap
                        .tabs
                        .iter()
                        .filter(|t| t.workspace_id == ws.workspace_id)
                    {
                        self.items.push(Item {
                            id: format!("tab:{}", tab.tab_id),
                            kind: "tab".into(),
                            workspace_id: ws.workspace_id.clone(),
                            tab_id: tab.tab_id.clone(),
                            pane_id: String::new(),
                            label: tab.label.clone(),
                        });
                        for pane in snap.panes.iter().filter(|p| p.tab_id == tab.tab_id) {
                            self.items.push(Item {
                                id: format!("pane:{}", pane.pane_id),
                                kind: "pane".into(),
                                workspace_id: ws.workspace_id.clone(),
                                tab_id: tab.tab_id.clone(),
                                pane_id: pane.pane_id.clone(),
                                label: pane.pane_id.clone(),
                            });
                        }
                    }
                }
            }
        }
        if self.view_mode != ViewMode::Tree {
            for ag in self.sorted_agents() {
                self.items.push(Item {
                    id: format!("ag:{}", ag.pane_id),
                    kind: "agent".into(),
                    workspace_id: ag.workspace_id,
                    tab_id: ag.tab_id,
                    pane_id: ag.pane_id,
                    label: ag.terminal_title_stripped,
                });
            }
        }
        self.cursor = self.cursor.min(self.items.len().saturating_sub(1));
    }
    pub fn sync_selection(&mut self) {
        if let Some(item) = self.items.get(self.cursor) {
            self.selected_workspace_id = item.workspace_id.clone();
            self.selected_tab_id = item.tab_id.clone();
            self.selected_pane_id = item.pane_id.clone();
        } else if let Some(snap) = &self.snapshot {
            // An agents-only view can legitimately be empty after a pane closes.
            self.selected_workspace_id = snap.focused_workspace_id.clone();
            self.selected_tab_id = snap.focused_tab_id.clone();
            self.selected_pane_id = snap.focused_pane_id.clone();
        }
    }
    pub fn target_pane(&self) -> String {
        let Some(snap) = &self.snapshot else {
            return String::new();
        };
        if !self.selected_pane_id.is_empty() {
            return self.selected_pane_id.clone();
        }
        for by_tab in [true, false] {
            let id = if by_tab {
                &self.selected_tab_id
            } else {
                &self.selected_workspace_id
            };
            if id.is_empty() {
                continue;
            }
            let matches = |p: &&dolphin_herdr::PaneInfo| {
                if by_tab {
                    p.tab_id == *id
                } else {
                    p.workspace_id == *id
                }
            };
            let candidates: Vec<_> = snap.panes.iter().filter(matches).collect();
            if let Some(pane) = candidates
                .iter()
                .find(|p| p.pane_id != self.self_pane_id && p.focused)
                .or_else(|| candidates.iter().find(|p| p.pane_id != self.self_pane_id))
                .or_else(|| candidates.first())
            {
                return pane.pane_id.clone();
            }
        }
        snap.focused_pane_id.clone()
    }
    pub fn workspace_context(&self) -> Option<WorkspaceContextPayload> {
        let snap = self.snapshot.as_ref()?;
        let pane_id = self.target_pane();
        let ws = snap
            .workspaces
            .iter()
            .find(|w| w.workspace_id == self.selected_workspace_id);
        let wt = ws.and_then(|w| w.worktree.as_ref());
        let mut cwd = wt.map(|w| w.checkout_path.clone()).unwrap_or_default();
        if let Some(pane) = snap.panes.iter().find(|p| p.pane_id == pane_id) {
            if !pane.foreground_cwd.is_empty() {
                cwd = pane.foreground_cwd.clone();
            } else if cwd.is_empty() {
                cwd = pane.cwd.clone();
            }
        }
        if cwd.is_empty() {
            cwd = home_dir().to_string_lossy().into_owned();
        }
        Some(WorkspaceContextPayload {
            workspace_id: self.selected_workspace_id.clone(),
            workspace_label: ws.map(|w| w.label.clone()).unwrap_or_default(),
            tab_id: self.selected_tab_id.clone(),
            pane_id,
            cwd,
            repo_name: wt.map(|w| w.repo_name.clone()).unwrap_or_default(),
        })
    }
    pub fn toast(&mut self, level: &str, title: &str, message: impl Into<String>) {
        self.outbox.push(Envelope::toast(ToastPayload {
            plugin_id: PLUGIN_ID.into(),
            level: level.into(),
            title: title.into(),
            message: message.into(),
        }));
    }
    pub fn handle_host(&mut self, env: Envelope) -> Vec<Effect> {
        match env.kind.as_str() {
            "welcome" => {
                if let Some(p) = env.welcome {
                    self.palette = p.palette;
                    self.slot_sizes.extend(p.slot_sizes);
                    self.focused_slot = p.focused_slot;
                    self.modifiers = p.modifiers;
                    self.render_all();
                }
            }
            "slot_resize" => {
                if let Some(p) = env.slot_resize {
                    self.slot_sizes.extend(p.slot_sizes);
                    self.focused_slot = p.focused_slot;
                    self.render_all();
                }
            }
            "theme_changed" => {
                if let Some(p) = env.theme_changed {
                    self.palette = p.palette;
                    self.render_all();
                }
            }
            "key_state" => {
                if let Some(p) = env.key_state {
                    self.modifiers = p.modifiers;
                    self.render_nav();
                    self.render_footer();
                    self.render_inspector();
                }
            }
            "slot_key" => {
                if let Some(p) = env.slot_key {
                    return self.key(&p.key);
                }
            }
            "slot_mouse" => {
                if let Some(p) = env.slot_mouse {
                    return self.mouse(p);
                }
            }
            "command_invoke" => {
                if let Some(p) = env.command_invoke {
                    return self.command(&p.command_id);
                }
            }
            _ => {}
        }
        Vec::new()
    }
    pub fn key(&mut self, key: &str) -> Vec<Effect> {
        let n = self.items.len();
        match key {
            "j" | "down" | "k" | "up" | "g" | "home" | "G" | "end" if n > 0 => {
                self.cursor = match key {
                    "j" | "down" => (self.cursor + 1) % n,
                    "k" | "up" => (self.cursor + n - 1) % n,
                    "g" | "home" => 0,
                    _ => n - 1,
                };
                self.sync_selection();
                self.render_all();
                return vec![Effect::Preview];
            }
            "enter" | "f" => return self.command("herdr.focus_selected"),
            "1" => return self.command("herdr.view.split"),
            "2" => return self.command("herdr.view.tree"),
            "3" => return self.command("herdr.view.agents"),
            "o" => return self.command("herdr.sort.toggle"),
            "r" => return self.command("herdr.refresh"),
            "n" => return self.command("herdr.workspace.create"),
            _ => {}
        }
        Vec::new()
    }
    pub fn mouse(&mut self, mouse: SlotMousePayload) -> Vec<Effect> {
        match mouse.button.as_str() {
            "wheel_down" => return self.key("j"),
            "wheel_up" => return self.key("k"),
            _ => {}
        }
        match mouse.hit_action.as_str() {
            "view_mode" => {
                if let Some(mode) = mouse.hit_payload.get("mode") {
                    return self.command(&format!("herdr.view.{mode}"));
                }
            }
            "toggle_sort" => return self.command("herdr.sort.toggle"),
            "focus_tab" => {
                if let Some(tab) = mouse.hit_payload.get("tab_id").filter(|s| !s.is_empty()) {
                    self.selected_tab_id = tab.clone();
                    return vec![Effect::FocusTab(tab.clone())];
                }
            }
            "select_item" => {
                if let Some(index) = self.items.iter().position(|it| it.id == mouse.hit_id) {
                    let was_selected = self.cursor == index;
                    self.cursor = index;
                    self.sync_selection();
                    self.render_all();
                    let mut effects = vec![Effect::Preview];
                    if was_selected {
                        effects.extend(self.command("herdr.focus_selected"));
                    }
                    return effects;
                }
            }
            _ => {}
        }
        Vec::new()
    }
    pub fn command(&mut self, id: &str) -> Vec<Effect> {
        match id {
            "herdr.focus_selected" => {
                return self
                    .items
                    .get(self.cursor)
                    .cloned()
                    .map(Effect::Focus)
                    .into_iter()
                    .collect()
            }
            "herdr.refresh" => return vec![Effect::Refresh],
            "herdr.workspace.create" => return vec![Effect::Create],
            "herdr.view.split" => self.view_mode = ViewMode::Split,
            "herdr.view.tree" => self.view_mode = ViewMode::Tree,
            "herdr.view.agents" => self.view_mode = ViewMode::Agents,
            "herdr.sort.toggle" => {
                self.sort_mode = if self.sort_mode == SortMode::Priority {
                    SortMode::Spaces
                } else {
                    SortMode::Priority
                };
                self.toast(
                    "info",
                    "Agent Sort Mode",
                    format!("Sorted by {}", self.sort_mode.name().to_uppercase()),
                );
            }
            _ => return Vec::new(),
        }
        self.rebuild_items();
        self.sync_selection();
        self.render_all();
        Vec::new()
    }
}
fn status_rank(status: &str) -> u8 {
    match status {
        "blocked" => 0,
        "working" => 1,
        "done" => 2,
        "idle" => 3,
        _ => 4,
    }
}

pub fn hello() -> Envelope {
    let commands: Vec<CommandSpec> =
        serde_json::from_str(include_str!("../commands.json")).expect("static command definitions");
    let descriptions = [
        "Split View",
        "Tree View",
        "Agents View",
        "Toggle Sort",
        "Refresh Herdr",
        "New Space",
    ];
    let command_ids = [
        "herdr.view.split",
        "herdr.view.tree",
        "herdr.view.agents",
        "herdr.sort.toggle",
        "herdr.refresh",
        "herdr.workspace.create",
    ];
    let keys = ["1", "2", "3", "o", "r", "n"];
    Envelope::hello(HelloPayload {
        plugin_id: PLUGIN_ID.into(),
        name: "Herdr Sidebar".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        language: "rust".into(),
        pid: std::process::id() as i32,
        description: "Live Herdr Spaces, Worktrees, Tabs, Panes & Agents sidebar + stage preview"
            .into(),
        slots: [
            SIDEBAR_NAVIGATION,
            SIDEBAR_LIST,
            THREAD_ROW_STATUS,
            SIDEBAR_FOOTER,
            MAIN_HEADER,
            MAIN_STAGE,
            SIDE_PANEL,
        ]
        .iter()
        .map(|s| (*s).into())
        .collect(),
        commands,
        keybindings: (0..keys.len())
            .map(|i| KeybindSpec {
                key: keys[i].into(),
                command_id: command_ids[i].into(),
                description: descriptions[i].into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dolphin_herdr::*;
    pub(crate) fn sidebar() -> Sidebar {
        let mut sidebar = Sidebar::new("/tmp/example.sock".into(), "self".into());
        sidebar.set_snapshot(SessionSnapshot {
            focused_workspace_id: "w1".into(),
            focused_tab_id: "t1".into(),
            focused_pane_id: "self".into(),
            workspaces: vec![WorkspaceInfo {
                workspace_id: "w1".into(),
                number: 1,
                label: "Example".into(),
                active_tab_id: "t1".into(),
                pane_count: 2,
                tab_count: 1,
                worktree: Some(WorktreeMetadata {
                    repo_name: "example".into(),
                    checkout_path: "/home/example/project".into(),
                    is_linked_worktree: true,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            tabs: vec![TabInfo {
                workspace_id: "w1".into(),
                tab_id: "t1".into(),
                label: "Example tab".into(),
                number: 1,
                pane_count: 2,
                ..Default::default()
            }],
            panes: vec![
                PaneInfo {
                    pane_id: "self".into(),
                    tab_id: "t1".into(),
                    workspace_id: "w1".into(),
                    focused: true,
                    ..Default::default()
                },
                PaneInfo {
                    pane_id: "p1".into(),
                    tab_id: "t1".into(),
                    workspace_id: "w1".into(),
                    foreground_cwd: "/home/example/foreground".into(),
                    terminal_title_stripped: "Agent".into(),
                    ..Default::default()
                },
            ],
            agents: vec![AgentInfo {
                pane_id: "p1".into(),
                tab_id: "t1".into(),
                workspace_id: "w1".into(),
                terminal_title_stripped: "Agent".into(),
                agent_status: "working".into(),
                tokens: BTreeMap::from([
                    ("context_used_pct".into(), "52".into()),
                    ("model".into(), "example-model".into()),
                ]),
                ..Default::default()
            }],
            ..Default::default()
        });
        sidebar
    }
    #[test]
    fn hello_matches_spike_registration() {
        let hello = hello().hello.unwrap();
        assert_eq!(hello.plugin_id, PLUGIN_ID);
        assert_eq!(hello.slots.len(), 7);
        assert_eq!(hello.commands.len(), 7);
        assert_eq!(hello.keybindings.len(), 6);
        assert!(hello.rail_items.is_empty());
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../husk-plugin.json")).unwrap();
        assert_eq!(manifest["id"], hello.plugin_id);
        assert_eq!(
            manifest["slots"],
            serde_json::to_value(hello.slots).unwrap()
        );
    }
    #[test]
    fn keys_cycle_and_change_views_without_losing_selection() {
        let mut sidebar = sidebar();
        assert_eq!(sidebar.items.len(), 2);
        assert_eq!(sidebar.target_pane(), "p1");
        assert_eq!(sidebar.key("k"), vec![Effect::Preview]);
        assert_eq!(sidebar.cursor, 1);
        assert_eq!(sidebar.key("j"), vec![Effect::Preview]);
        assert_eq!(sidebar.cursor, 0);
        sidebar.key("2");
        assert_eq!(sidebar.view_mode, ViewMode::Tree);
        assert_eq!(sidebar.items.len(), 4);
        sidebar.key("G");
        assert_eq!(sidebar.selected_pane_id, "p1");
        sidebar.key("g");
        assert_eq!(sidebar.cursor, 0);
        sidebar.key("3");
        assert_eq!(sidebar.items.len(), 1);
        assert_eq!(sidebar.items[0].kind, "agent");
        assert!(
            matches!(sidebar.key("enter").as_slice(),[Effect::Focus(item)] if item.pane_id=="p1")
        );
        assert_eq!(sidebar.key("r"), vec![Effect::Refresh]);
        assert_eq!(sidebar.key("n"), vec![Effect::Create]);
    }
    #[test]
    fn mouse_selection_requires_second_click_to_focus() {
        let mut sidebar = sidebar();
        let mouse = SlotMousePayload {
            hit_id: "ag:p1".into(),
            hit_action: "select_item".into(),
            ..Default::default()
        };
        assert_eq!(sidebar.mouse(mouse.clone()), vec![Effect::Preview]);
        assert!(matches!(
            sidebar.mouse(mouse).as_slice(),
            [Effect::Preview, Effect::Focus(_)]
        ));
        let effects = sidebar.mouse(SlotMousePayload {
            hit_action: "focus_tab".into(),
            hit_payload: BTreeMap::from([("tab_id".into(), "t1".into())]),
            ..Default::default()
        });
        assert_eq!(effects, vec![Effect::FocusTab("t1".into())]);
        sidebar.mouse(SlotMousePayload {
            button: "wheel_up".into(),
            ..Default::default()
        });
        assert_eq!(sidebar.cursor, 0);
    }
    #[test]
    fn context_prefers_foreground_cwd_then_checkout_then_home() {
        let mut sidebar = sidebar();
        assert_eq!(
            sidebar.workspace_context().unwrap().cwd,
            "/home/example/foreground"
        );
        let snap = sidebar.snapshot.as_mut().unwrap();
        snap.panes.clear();
        assert_eq!(
            sidebar.workspace_context().unwrap().cwd,
            "/home/example/project"
        );
        sidebar.snapshot.as_mut().unwrap().workspaces[0].worktree = None;
        assert_eq!(
            sidebar.workspace_context().unwrap().cwd,
            home_dir().to_string_lossy()
        );
    }
    #[test]
    fn sorting_is_priority_then_workspace_then_pane() {
        let mut sidebar = sidebar();
        let snap = sidebar.snapshot.as_mut().unwrap();
        snap.agents.push(AgentInfo {
            pane_id: "p2".into(),
            workspace_id: "w1".into(),
            agent_status: "blocked".into(),
            ..Default::default()
        });
        assert_eq!(sidebar.sorted_agents()[0].pane_id, "p2");
        sidebar.key("o");
        assert_eq!(sidebar.sort_mode, SortMode::Spaces);
        assert_eq!(sidebar.sorted_agents()[0].pane_id, "p1");
        assert!(sidebar.outbox.iter().any(|e| e
            .toast
            .as_ref()
            .is_some_and(|t| t.title == "Agent Sort Mode")));
    }
    #[test]
    fn resize_and_modifiers_update_rendering() {
        let mut sidebar = sidebar();
        sidebar.handle_host(Envelope {
            kind: "slot_resize".into(),
            slot_resize: Some(SlotResizePayload {
                slot_sizes: sizes(&[(SIDEBAR_LIST, 26, 6)]),
                focused_slot: MAIN_STAGE.into(),
            }),
            ..Default::default()
        });
        assert_eq!(sidebar.focused_slot, MAIN_STAGE);
        assert_eq!(sidebar.slot_sizes[SIDEBAR_LIST].height, 6);
        sidebar.outbox.clear();
        sidebar.handle_host(Envelope {
            kind: "key_state".into(),
            key_state: Some(KeyStatePayload {
                modifiers: ModifierState {
                    alt: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        });
        assert!(sidebar.modifiers.alt);
        assert!(sidebar
            .outbox
            .iter()
            .filter_map(|e| e.slot_update.as_ref())
            .flat_map(|s| &s.lines)
            .any(|s| s.contains("ALT HELD")));
    }
    #[test]
    fn empty_snapshot_never_underflows_navigation() {
        let mut sidebar = Sidebar::new(String::new(), String::new());
        sidebar.set_snapshot(SessionSnapshot::default());
        for key in ["j", "k", "g", "G", "1", "2", "3", "enter"] {
            assert!(sidebar.key(key).is_empty());
        }
        sidebar.render_all();
        assert!(sidebar.items.is_empty());
    }
}
