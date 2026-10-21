use crate::{Sidebar, ViewMode, FRAMES, PLUGIN_ID};
use dolphin_herdr::{AgentInfo, PaneInfo, TabInfo, WorkspaceInfo};
use husk_plugin::*;
use unicode_width::UnicodeWidthChar;

fn tokens(text: &str) -> Vec<(&str, usize)> {
    let mut out = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let tail = &text[offset..];
        if tail.starts_with("\x1b[") {
            if let Some(end) = tail
                .as_bytes()
                .iter()
                .enumerate()
                .skip(2)
                .find(|(_, c)| (0x40..=0x7e).contains(*c))
                .map(|(i, _)| i + 1)
            {
                out.push((&tail[..end], 0));
                offset += end;
                continue;
            }
        }
        let c = tail.chars().next().expect("nonempty string");
        out.push((
            &tail[..c.len_utf8()],
            UnicodeWidthChar::width(c).unwrap_or(0),
        ));
        offset += c.len_utf8();
    }
    out
}
fn visible_width(s: &str) -> usize {
    tokens(s).iter().map(|(_, w)| w).sum()
}
/// Fit ANSI-colored text to terminal cells without cutting UTF-8 or SGR sequences.
pub fn fit_line(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let parts = tokens(text);
    let clipped = parts.iter().map(|(_, w)| w).sum::<usize>() > width;
    let limit = if clipped { width - 1 } else { width };
    let mut used = 0;
    let mut result = String::new();
    for (part, w) in parts {
        if used + w > limit {
            break;
        }
        result.push_str(part);
        used += w;
    }
    if clipped {
        result.push('…');
        used += 1;
    }
    result.push_str(&" ".repeat(width - used));
    if text.contains('\x1b') {
        result.push_str("\x1b[0m");
    }
    result
}
fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..], 16).ok()?,
    ))
}
fn fg(text: impl AsRef<str>, color: &str) -> String {
    let text = text.as_ref();
    if let Some((r, g, b)) = rgb(color) {
        format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[39m")
    } else {
        text.into()
    }
}
fn bg(text: &str, color: &str) -> String {
    if let Some((r, g, b)) = rgb(color) {
        format!("\x1b[48;2;{r};{g};{b}m{text}\x1b[49m")
    } else {
        text.into()
    }
}
fn bold(text: impl AsRef<str>) -> String {
    format!("\x1b[1m{}\x1b[22m", text.as_ref())
}
fn join_lr(left: &str, right: &str, width: usize) -> String {
    format!(
        "{left}{}{right}",
        " ".repeat(
            width
                .saturating_sub(visible_width(left) + visible_width(right))
                .max(1)
        )
    )
}
fn pad(lines: Vec<String>, width: usize, height: usize) -> Vec<String> {
    lines
        .into_iter()
        .chain(std::iter::repeat(String::new()))
        .take(height)
        .map(|s| fit_line(&s, width))
        .collect()
}
fn hit(line: usize, id: &str, kind: &str, action: &str) -> RowHitTarget {
    RowHitTarget {
        line: line as i32,
        id: id.into(),
        kind: kind.into(),
        action: action.into(),
        ..Default::default()
    }
}
fn base(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}
fn first<'a>(values: &[&'a str]) -> &'a str {
    values.iter().copied().find(|v| !v.is_empty()).unwrap_or("")
}

impl Sidebar {
    fn dimensions(&self, slot: &str, min_w: usize, min_h: usize) -> (usize, usize) {
        let sz = self.slot_sizes.get(slot).cloned().unwrap_or_default();
        (
            (sz.width.max(0) as usize).max(min_w).min(4096),
            (sz.height.max(0) as usize).max(min_h).min(4096),
        )
    }
    fn update(
        &mut self,
        slot: &str,
        title: &str,
        badge: impl Into<String>,
        lines: Vec<String>,
        hits: Vec<RowHitTarget>,
    ) {
        self.outbox.push(Envelope::slot_update(SlotUpdatePayload {
            plugin_id: PLUGIN_ID.into(),
            slot: slot.into(),
            title: title.into(),
            badge: badge.into(),
            lines,
            hit_targets: hits,
            ..Default::default()
        }));
    }
    pub fn render_all(&mut self) {
        if let Some(context) = self.workspace_context() {
            self.outbox.push(Envelope::workspace_context(context));
        }
        self.render_nav();
        self.render_list();
        self.render_footer();
        self.render_header();
        self.render_stage();
        self.render_inspector();
    }
    fn glyph(&self, status: &str, fallback: &str) -> String {
        match status {
            "blocked" => bold(fg("●", &self.palette.danger)),
            "working" => bold(fg(
                FRAMES[self.spinner_frame % FRAMES.len()],
                &self.palette.success,
            )),
            "done" => bold(fg("✓", &self.palette.info)),
            _ => fg("○", fallback),
        }
    }
    fn selected(&self, row: &str, selected: bool, width: usize) -> String {
        let row = fit_line(row, width);
        if selected {
            bg(&row, &self.palette.surface_selected)
        } else {
            row
        }
    }
    pub fn render_nav(&mut self) {
        let (w, _) = self.dimensions(SIDEBAR_NAVIGATION, 20, 2);
        let mut pills = Vec::new();
        for (key, label, mode) in [
            ("1", "Spaces", ViewMode::Split),
            ("2", "Tree", ViewMode::Tree),
            ("3", "Agents", ViewMode::Agents),
        ] {
            let text = if self.modifiers.alt {
                format!(" [{key}] {label} ")
            } else {
                format!(" {key} {label} ")
            };
            pills.push(if self.view_mode == mode {
                bg(&bold(fg(text, &self.palette.canvas)), &self.palette.accent)
            } else {
                bg(
                    &fg(
                        text,
                        if self.modifiers.alt {
                            &self.palette.warning
                        } else {
                            &self.palette.ink_muted
                        },
                    ),
                    &self.palette.surface_elevated,
                )
            });
        }
        let (ws, agents, working, blocked) = self
            .snapshot
            .as_ref()
            .map(|s| {
                (
                    s.workspaces.len(),
                    s.agents.len(),
                    s.agents
                        .iter()
                        .filter(|a| a.agent_status == "working")
                        .count(),
                    s.agents
                        .iter()
                        .filter(|a| a.agent_status == "blocked")
                        .count(),
                )
            })
            .unwrap_or_default();
        let mut summary = fg(format!(" {ws} sp · {agents} ag"), &self.palette.ink_subtle);
        if working > 0 {
            summary += &fg(format!(" · {working}⚡"), &self.palette.success);
        }
        if blocked > 0 {
            summary += &fg(format!(" · {blocked}!"), &self.palette.warning);
        }
        let sort = fg(
            format!("o:{} ", self.sort_mode.name()),
            &self.palette.accent_secondary,
        );
        let mut view_hit = hit(0, "nav-tabs", "nav", "view_mode");
        view_hit
            .payload
            .insert("mode".into(), self.view_mode.next().name().into());
        self.update(
            SIDEBAR_NAVIGATION,
            "Navigation",
            self.view_mode.name().to_uppercase(),
            vec![
                fit_line(&pills.join(" "), w),
                fit_line(&join_lr(&summary, &sort, w), w),
            ],
            vec![view_hit, hit(1, "nav-sort", "nav", "toggle_sort")],
        );
    }
    fn workspace_row(&self, ws: &WorkspaceInfo, selected: bool, w: usize) -> String {
        let color = self.palette.space_color(ws.space_color_key());
        let bar = if selected {
            bold(fg("▎", &self.palette.accent))
        } else if ws.focused {
            bold(fg("•", color))
        } else {
            " ".into()
        };
        let prefix = format!(
            "{}{} {} ",
            bar,
            fg(format!("{:2}", ws.number), &self.palette.ink_subtle),
            self.glyph(&ws.agent_status, color)
        );
        let mut badges = Vec::new();
        if let Some(wt) = &ws.worktree {
            if wt.is_linked_worktree {
                badges.push(fg("⎇ wt", &self.palette.info));
            } else if !wt.repo_name.is_empty() && !wt.repo_name.eq_ignore_ascii_case(&ws.label) {
                badges.push(fg(format!("⎇ {}", wt.repo_name), &self.palette.ink_subtle));
            }
        }
        badges.push(fg(format!("{}p", ws.pane_count), &self.palette.ink_subtle));
        let right = badges.join(" ");
        let avail = w
            .saturating_sub(visible_width(&prefix) + visible_width(&right) + 2)
            .max(4);
        let label = fit_line(&ws.label, avail).trim_end().to_string();
        let label = fg(label, if ws.focused { color } else { &self.palette.ink });
        let label = if selected || ws.focused {
            bold(label)
        } else {
            label
        };
        self.selected(&join_lr(&(prefix + &label), &right, w), selected, w)
    }
    fn tab_row(&self, tab: &TabInfo, selected: bool, w: usize) -> String {
        let prefix = if selected {
            format!("{}  ├─ ", bold(fg("▎", &self.palette.accent)))
        } else {
            "   ├─ ".into()
        };
        let label = fg(
            format!("tab {}: {}", tab.number, tab.label),
            if tab.focused {
                &self.palette.accent_secondary
            } else {
                &self.palette.ink_muted
            },
        );
        let right = fg(format!("{}p", tab.pane_count), &self.palette.ink_subtle);
        self.selected(&join_lr(&(prefix + &label), &right, w), selected, w)
    }
    fn pane_row(&self, pane: &PaneInfo, selected: bool, w: usize) -> String {
        let prefix = if selected {
            format!("{}  │  └─ ", bold(fg("▎", &self.palette.accent)))
        } else {
            "   │  └─ ".into()
        };
        let title = first(&[&pane.terminal_title_stripped, base(&pane.cwd)]);
        let row = format!(
            "{}{} {}",
            prefix,
            self.glyph(&pane.agent_status, &self.palette.info),
            fg(
                fit_line(title, w.saturating_sub(16).max(6)).trim_end(),
                &self.palette.ink_muted
            )
        );
        self.selected(&row, selected, w)
    }
    fn gauge(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        if let Ok(pct) = text.trim().trim_end_matches('%').parse::<i32>() {
            let color = if pct >= 80 {
                &self.palette.danger
            } else if pct >= 50 {
                &self.palette.warning
            } else {
                &self.palette.success
            };
            let filled = (pct.saturating_add(24) / 25).clamp(1, 4) as usize;
            fg(
                format!("{}{} {pct}%", "▰".repeat(filled), "▱".repeat(4 - filled)),
                color,
            )
        } else {
            fg(text, &self.palette.ink_subtle)
        }
    }
    fn agent_rows(
        &self,
        ag: &AgentInfo,
        ws: &WorkspaceInfo,
        selected: bool,
        w: usize,
    ) -> [String; 2] {
        let color = self.palette.space_color(ws.space_color_key());
        let bar = if selected {
            bold(fg("▎", &self.palette.accent))
        } else if ag.focused {
            bold(fg("•", color))
        } else {
            " ".into()
        };
        let prefix = format!("{}{} ", bar, self.glyph(&ag.agent_status, color));
        let title = first(&[&ag.terminal_title_stripped, &ag.title, &ws.label]);
        let gauge = self.gauge(
            ag.tokens
                .get("context_used_pct")
                .map(String::as_str)
                .unwrap_or_default(),
        );
        let right = format!(
            "{}{}{}",
            gauge,
            if gauge.is_empty() { "" } else { " " },
            fg(format!("#{}", ws.number), color)
        );
        let avail = w
            .saturating_sub(visible_width(&prefix) + visible_width(&right) + 2)
            .max(6);
        let title = fg(fit_line(title, avail).trim_end(), &self.palette.ink);
        let title = if selected || ag.focused {
            bold(title)
        } else {
            title
        };
        let line1 = join_lr(&(prefix + &title), &right, w);
        let model = first(&[
            ag.tokens
                .get("model_effort")
                .map(String::as_str)
                .unwrap_or_default(),
            ag.tokens
                .get("model")
                .map(String::as_str)
                .unwrap_or_default(),
            &ag.agent,
        ]);
        let status = match ag.agent_status.as_str() {
            "blocked" => bold(fg("blocked", &self.palette.danger)),
            "working" => fg("working", &self.palette.success),
            "done" => bold(fg("done", &self.palette.info)),
            _ => fg("idle", &self.palette.ink_subtle),
        };
        let model = format!(
            "   {}",
            fg(
                fit_line(model, w.saturating_sub(12).max(8)).trim_end(),
                &self.palette.ink_subtle
            )
        );
        [
            self.selected(&line1, selected, w),
            self.selected(&join_lr(&model, &status, w), selected, w),
        ]
    }
    pub fn render_list(&mut self) {
        let (w, h) = self.dimensions(SIDEBAR_LIST, 22, 6);
        if !self.connected || self.snapshot.is_none() {
            self.update(
                SIDEBAR_LIST,
                "Herdr Spaces & Agents",
                "OFFLINE",
                pad(
                    vec![
                        bold(fg("⚠ Herdr Disconnected", &self.palette.danger)),
                        fg(&self.socket_path, &self.palette.ink_muted),
                        fg(&self.last_error, &self.palette.ink_muted),
                        String::new(),
                        fg("Press 'r' to reconnect", &self.palette.ink_muted),
                    ],
                    w,
                    h,
                ),
                Vec::new(),
            );
            return;
        }
        let snap = self.snapshot.as_ref().expect("checked");
        let mut lines = Vec::new();
        let mut hits = Vec::new();
        let mut cursor_line = 0;
        let mut item_index = 0;
        let section = |s: String| bold(fg(s, &self.palette.ink_subtle));
        match self.view_mode {
            ViewMode::Split | ViewMode::Tree => {
                lines.push(section(if self.view_mode == ViewMode::Split {
                    format!("SPACES ({})", snap.workspaces.len())
                } else {
                    "SESSION TOPOLOGY TREE".into()
                }));
                for ws in &snap.workspaces {
                    let selected = item_index == self.cursor;
                    if selected {
                        cursor_line = lines.len();
                    }
                    hits.push(hit(
                        lines.len(),
                        &format!("ws:{}", ws.workspace_id),
                        "workspace",
                        "select_item",
                    ));
                    lines.push(self.workspace_row(ws, selected, w));
                    item_index += 1;
                    if self.view_mode == ViewMode::Tree {
                        for tab in snap
                            .tabs
                            .iter()
                            .filter(|t| t.workspace_id == ws.workspace_id)
                        {
                            let selected = item_index == self.cursor;
                            if selected {
                                cursor_line = lines.len();
                            }
                            hits.push(hit(
                                lines.len(),
                                &format!("tab:{}", tab.tab_id),
                                "tab",
                                "select_item",
                            ));
                            lines.push(self.tab_row(tab, selected, w));
                            item_index += 1;
                            for pane in snap.panes.iter().filter(|p| p.tab_id == tab.tab_id) {
                                let selected = item_index == self.cursor;
                                if selected {
                                    cursor_line = lines.len();
                                }
                                hits.push(hit(
                                    lines.len(),
                                    &format!("pane:{}", pane.pane_id),
                                    "pane",
                                    "select_item",
                                ));
                                lines.push(self.pane_row(pane, selected, w));
                                item_index += 1;
                            }
                        }
                    }
                }
                if self.view_mode == ViewMode::Split {
                    lines.push(String::new());
                    lines.push(section(format!(
                        "AGENTS ({}) · {}",
                        snap.agents.len(),
                        self.sort_mode.name().to_uppercase()
                    )));
                }
            }
            ViewMode::Agents => lines.push(section(format!(
                "AGENT MISSION CONTROL ({})",
                snap.agents.len()
            ))),
        }
        if self.view_mode != ViewMode::Tree {
            for ag in self.sorted_agents() {
                let ws = snap
                    .workspaces
                    .iter()
                    .find(|w| w.workspace_id == ag.workspace_id)
                    .cloned()
                    .unwrap_or_default();
                let selected = item_index == self.cursor;
                if selected {
                    cursor_line = lines.len();
                }
                for offset in [0, 1] {
                    hits.push(hit(
                        lines.len() + offset,
                        &format!("ag:{}", ag.pane_id),
                        "agent",
                        "select_item",
                    ));
                }
                lines.extend(self.agent_rows(&ag, &ws, selected, w));
                item_index += 1;
            }
        }
        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
        } else if cursor_line >= self.scroll_offset + h {
            self.scroll_offset = cursor_line.saturating_sub(h).saturating_add(2);
        }
        self.scroll_offset = self.scroll_offset.min(lines.len().saturating_sub(h));
        let hits = hits
            .into_iter()
            .filter_map(|mut ht| {
                let line = ht.line as usize;
                if line < self.scroll_offset || line >= self.scroll_offset + h {
                    return None;
                }
                ht.line = (line - self.scroll_offset) as i32;
                Some(ht)
            })
            .collect();
        let visible = pad(
            lines.into_iter().skip(self.scroll_offset).take(h).collect(),
            w,
            h,
        );
        self.update(
            SIDEBAR_LIST,
            "Spaces & Agents",
            format!("{}/{}", self.cursor + 1, self.items.len().max(1)),
            visible,
            hits,
        );
    }
    pub fn render_footer(&mut self) {
        let (w, _) = self.dimensions(SIDEBAR_FOOTER, 20, 2);
        let state = if self.connected {
            self.snapshot
                .as_ref()
                .map(|s| {
                    format!(
                        "herdr v{} · proto {} · ev #{}",
                        s.version, s.protocol, self.event_count
                    )
                })
                .unwrap_or_default()
        } else {
            "disconnected from herdr.sock".into()
        };
        let line1 = format!(
            "{} {}",
            bold(fg(
                "●",
                if self.connected {
                    &self.palette.success
                } else {
                    &self.palette.danger
                }
            )),
            fg(state, &self.palette.ink_muted)
        );
        let line2 = if self.modifiers.alt {
            bold(fg(
                "⌥ ALT HELD · [1-3]View [o]Sort [⏎]Focus [n]New",
                &self.palette.warning,
            ))
        } else {
            fg(
                "⏎ focus herdr · j/k nav · o sort · n new",
                &self.palette.ink_subtle,
            )
        };
        self.update(
            SIDEBAR_FOOTER,
            "Daemon Link",
            format!("PID {}", std::process::id()),
            vec![fit_line(&line1, w), fit_line(&line2, w)],
            Vec::new(),
        );
    }
    pub fn render_header(&mut self) {
        let (w, _) = self.dimensions(MAIN_HEADER, 40, 2);
        let Some(snap) = &self.snapshot else { return };
        let Some(ws) = snap
            .workspaces
            .iter()
            .find(|w| w.workspace_id == self.selected_workspace_id)
            .or_else(|| snap.workspaces.first())
        else {
            return;
        };
        let color = self.palette.space_color(ws.space_color_key());
        let pill = bg(
            &bold(fg(
                format!(" #{} {} ", ws.number, ws.label),
                &self.palette.canvas,
            )),
            color,
        );
        let wt = ws
            .worktree
            .as_ref()
            .map(|wt| {
                fg(
                    format!(
                        " ⎇ repo:{}{}",
                        wt.repo_name,
                        if wt.is_linked_worktree {
                            " (linked worktree)"
                        } else {
                            ""
                        }
                    ),
                    &self.palette.info,
                )
            })
            .unwrap_or_default();
        let mut pills = Vec::new();
        for tab in snap
            .tabs
            .iter()
            .filter(|t| t.workspace_id == ws.workspace_id)
        {
            let current = tab.tab_id == self.selected_tab_id
                || (self.selected_tab_id.is_empty() && tab.tab_id == ws.active_tab_id);
            let text = fg(
                format!(" Tab {}: {} ", tab.number, tab.label),
                if current {
                    &self.palette.ink
                } else {
                    &self.palette.ink_subtle
                },
            );
            pills.push(bg(
                &text,
                if current {
                    &self.palette.surface_selected
                } else {
                    &self.palette.surface_elevated
                },
            ));
        }
        let row = format!("{pill}{wt}   {}", pills.join(" "));
        let sub=fg(format!("Workspace {} · {} panes · Active Tab {} · Press Enter in sidebar to focus in Herdr",ws.workspace_id,ws.pane_count,ws.active_tab_id),&self.palette.ink_subtle);
        let title = ws.label.clone();
        let badge = ws.workspace_id.clone();
        self.update(
            MAIN_HEADER,
            &title,
            badge,
            vec![fit_line(&row, w), fit_line(&sub, w)],
            Vec::new(),
        );
    }
    pub fn render_stage(&mut self) {
        let (w, h) = self.dimensions(MAIN_STAGE, 40, 12);
        let Some(snap) = &self.snapshot else { return };
        let active = snap
            .workspaces
            .iter()
            .find(|w| w.workspace_id == self.selected_workspace_id)
            .or_else(|| snap.workspaces.first());
        let pane = self.target_pane();
        let title = active
            .map(|ws| {
                format!(
                    "SPACE #{} · {} ({})",
                    ws.number,
                    ws.label.to_uppercase(),
                    ws.workspace_id
                )
            })
            .unwrap_or_else(|| "WORKSPACE TOPOLOGY & AGENT DECK".into());
        let section = |text: &str| bold(fg(text, &self.palette.accent));
        let subtle = |text: &str| fg(text, &self.palette.ink_subtle);
        let mut lines = vec![join_lr(
            &section(&title),
            &subtle("press 'v' for Husk Slot Map"),
            w,
        )];
        if let Some(layout) = self
            .preview_layout
            .as_ref()
            .filter(|_| self.preview_pane_id == pane)
        {
            if !layout.panes.is_empty() {
                let mode = if layout.layout_mode.is_empty() {
                    "tiled"
                } else {
                    &layout.layout_mode
                };
                let chips: Vec<_> = layout
                    .panes
                    .iter()
                    .map(|p| {
                        let chip = format!("[{} {}x{}]", p.pane_id, p.width, p.height);
                        if p.pane_id == pane {
                            bold(fg(chip, &self.palette.success))
                        } else {
                            subtle(&chip)
                        }
                    })
                    .collect();
                lines.push(format!(
                    "{}{}",
                    subtle(&format!("BSP Geometry ({mode}): ")),
                    chips.join(" ")
                ));
            }
        }
        lines.push(subtle(&"─".repeat(w)));
        if let Some(wt) = active.and_then(|ws| ws.worktree.as_ref()) {
            let kind = if wt.is_linked_worktree {
                "Linked Git Worktree (⎇)"
            } else {
                "Primary Checkout"
            };
            lines.extend([
                section("GIT WORKTREE"),
                format!(
                    "  {}{}  {}",
                    subtle("Repo:     "),
                    fg(&wt.repo_name, &self.palette.ink),
                    fg(format!("({kind})"), &self.palette.ink_muted)
                ),
                format!(
                    "  {}{}",
                    subtle("Checkout: "),
                    fg(&wt.checkout_path, &self.palette.ink_muted)
                ),
                String::new(),
            ]);
        }
        lines.push(section("TABS & PANES IN THIS SPACE"));
        for tab in &snap.tabs {
            if active.is_some_and(|ws| tab.workspace_id != ws.workspace_id) {
                continue;
            }
            lines.push(format!(
                "  {}",
                bold(fg(
                    format!("▾ Tab {}: {} ({})", tab.number, tab.label, tab.tab_id),
                    &self.palette.accent_secondary
                ))
            ));
            for p in snap.panes.iter().filter(|p| p.tab_id == tab.tab_id) {
                let selected = p.pane_id == pane;
                let pointer = if selected {
                    format!("  {} ├─ ", bold(fg("▸", &self.palette.accent)))
                } else {
                    "    ├─ ".into()
                };
                let agent = if p.agent.is_empty() {
                    "shell"
                } else {
                    &p.agent
                };
                let title = first(&[&p.terminal_title_stripped, &p.cwd]);
                let row = format!(
                    "{}{} {:8} [{}] {}",
                    pointer,
                    self.glyph(&p.agent_status, &self.palette.info),
                    p.pane_id,
                    agent,
                    title
                );
                lines.push(self.selected(&fg(row, &self.palette.ink), selected, w));
                let mut meta = Vec::new();
                if !p.foreground_cwd.is_empty() {
                    meta.push(format!("cwd: {}", p.foreground_cwd));
                }
                if let Some(model) = p.tokens.get("model_effort").filter(|s| !s.is_empty()) {
                    meta.push(format!("model: {model}"));
                }
                if let Some(ctx) = p.tokens.get("context_compact").filter(|s| !s.is_empty()) {
                    meta.push(format!("ctx: {ctx}"));
                }
                if !meta.is_empty() {
                    lines.push(format!("         {}", subtle(&meta.join(" · "))));
                }
            }
            lines.push(String::new());
        }
        self.update(
            MAIN_STAGE,
            &format!("Stage · {pane}"),
            "TOPOLOGY DECK",
            pad(lines, w, h),
            Vec::new(),
        );
    }
    pub fn render_inspector(&mut self) {
        let (w, h) = self.dimensions(SIDE_PANEL, 24, 10);
        let Some(snap) = &self.snapshot else { return };
        let pane_id = self.target_pane();
        let pane = snap.panes.iter().find(|p| p.pane_id == pane_id);
        let ws = snap
            .workspaces
            .iter()
            .find(|ws| ws.workspace_id == self.selected_workspace_id);
        let section = |s: &str| bold(fg(s, &self.palette.accent));
        let kv = |k: &str, v: String| {
            let key = fg(
                format!("{:<11} ", format!("{k}:")),
                &self.palette.ink_subtle,
            );
            let val = fit_line(&v, w.saturating_sub(visible_width(&key)).max(6));
            format!("{}{}", key, fg(val.trim_end(), &self.palette.ink))
        };
        let mut lines = vec![section("SELECTED SPACE")];
        if let Some(ws) = ws {
            lines.extend([
                kv(
                    "Space",
                    format!("#{} {} ({})", ws.number, ws.label, ws.workspace_id),
                ),
                kv("Status", ws.agent_status.clone()),
                kv(
                    "Topology",
                    format!("{} tabs · {} panes", ws.tab_count, ws.pane_count),
                ),
            ]);
            if let Some(wt) = &ws.worktree {
                lines.extend([
                    kv("Repo", wt.repo_name.clone()),
                    kv("Linked WT", wt.is_linked_worktree.to_string()),
                    kv("Checkout", wt.checkout_path.clone()),
                ]);
            }
        }
        lines.extend([String::new(), section("PANE & AGENT INSPECTOR")]);
        if let Some(p) = pane {
            let terminal = p
                .terminal_id
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| p.terminal_id.to_string());
            lines.extend([
                kv("Pane ID", p.pane_id.clone()),
                kv("Terminal", terminal),
                kv("Agent", p.agent.clone()),
                kv("Status", p.agent_status.clone()),
                kv("Title", p.terminal_title_stripped.clone()),
                kv("CWD", p.foreground_cwd.clone()),
            ]);
            if let Some(session) = &p.agent_session {
                lines.extend([
                    kv("Session", session.value.clone()),
                    kv("Source", session.source.clone()),
                ]);
            }
            if !p.tokens.is_empty() {
                lines.extend([String::new(), section("AGENT TOKENS")]);
                for key in [
                    "model",
                    "effort",
                    "context_used_pct",
                    "context_used",
                    "context_left",
                    "context_window",
                ] {
                    if let Some(value) = p.tokens.get(key).filter(|s| !s.is_empty()) {
                        lines.push(kv(key, value.clone()));
                    }
                }
            }
        }
        lines.extend([
            String::new(),
            section("OUT-OF-PROCESS IPC"),
            kv("Plugin PID", format!("{} (Rust)", std::process::id())),
            kv(
                "Herdr Evts",
                format!("{} ({})", self.event_count, self.last_event),
            ),
            kv("Theme", self.palette.name.clone()),
        ]);
        self.update(
            SIDE_PANEL,
            "Inspector",
            pane_id,
            pad(lines, w, h),
            Vec::new(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_ansi_width_and_padding() {
        for width in 0..20 {
            let fitted = fit_line("\x1b[31m日本語 agent ⚡\x1b[0m", width);
            assert_eq!(visible_width(&fitted), width);
            assert!(!fitted.contains('\u{fffd}'));
        }
        assert_eq!(fit_line("abcdef", 4), "abc…");
        assert_eq!(fit_line("a", 3), "a  ");
        assert_eq!(visible_width("日本"), 4);
    }
    #[test]
    fn stage_never_renders_geometry_from_the_previous_selection() {
        let mut sidebar = crate::tests::sidebar();
        sidebar.preview_layout = Some(dolphin_herdr::PaneLayoutResult {
            panes: vec![dolphin_herdr::PaneLayoutRect {
                pane_id: "old-pane".into(),
                width: 80,
                height: 24,
                ..Default::default()
            }],
            ..Default::default()
        });
        sidebar.preview_pane_id = "previous-target".into();
        sidebar.render_stage();
        assert!(!sidebar
            .outbox
            .iter()
            .filter_map(|e| e.slot_update.as_ref())
            .flat_map(|p| &p.lines)
            .any(|line| line.contains("BSP Geometry")));
        sidebar.outbox.clear();
        sidebar.preview_pane_id = sidebar.target_pane();
        sidebar.render_stage();
        assert!(sidebar
            .outbox
            .iter()
            .filter_map(|e| e.slot_update.as_ref())
            .flat_map(|p| &p.lines)
            .any(|line| line.contains("BSP Geometry")));
    }
    #[test]
    fn all_slots_and_hit_targets_are_bounded() {
        let mut sidebar = crate::tests::sidebar();
        sidebar.render_all();
        assert!(sidebar.outbox.iter().any(|e| e.workspace_context.is_some()));
        let slots: Vec<_> = sidebar
            .outbox
            .iter()
            .filter_map(|e| e.slot_update.as_ref())
            .collect();
        assert_eq!(slots.len(), 6); // thread-row-status is advertised but not emitted by the spike.
        for slot in slots {
            for line in &slot.lines {
                assert!(visible_width(line) >= 20);
            }
            for target in &slot.hit_targets {
                assert!(target.line >= 0 && (target.line as usize) < slot.lines.len());
            }
        }
    }
}
