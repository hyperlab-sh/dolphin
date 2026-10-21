use anyhow::{bail, Context, Result};
use dolphin_herdr::Client;
use herdr_sidebar::{runtime, Sidebar};
use std::env;

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
    let (reader, writer) = husk_plugin::connect().await?;
    let sidebar = Sidebar::new(
        client.socket_path.to_string_lossy().into_owned(),
        env::var("HERDR_PANE_ID").unwrap_or_default(),
    );
    runtime::run(sidebar, client, reader, writer).await
}
