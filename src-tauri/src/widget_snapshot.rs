//! Sanitised data bridge for the native macOS WidgetKit extension.
//!
//! The widget never reads Metrik's SQLite ledger directly. The host app publishes a
//! compact, versioned JSON snapshot containing only derived totals and official quota
//! metadata. This keeps storage ownership in the shared core and gives WidgetKit a
//! stable contract that can evolve independently from the database schema.

use crate::domain::UsageSnapshot;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const SNAPSHOT_FILE_NAME: &str = "widget-snapshot.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WidgetSnapshot<'a> {
    schema_version: u8,
    generated_at: &'a str,
    total_tokens: i64,
    agents: Vec<WidgetAgent<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WidgetAgent<'a> {
    id: &'a str,
    label: &'static str,
    tokens: i64,
    windows: Vec<WidgetQuotaWindow<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WidgetQuotaWindow<'a> {
    key: &'a str,
    label: &'a str,
    available: bool,
    remaining_percent: f64,
    resets_in_minutes: Option<f64>,
    stale: bool,
    reset_expired: bool,
    quality: &'a str,
}

fn agent_label(id: &str) -> &'static str {
    match id {
        "codex" => "ChatGPT",
        "claude" => "Claude",
        "zcode" => "GLM",
        "opencode" => "OpenCode",
        "kimi" => "Kimi",
        "antigravity" => "Antigravity",
        _ => "Agent",
    }
}

fn make_payload(snapshot: &UsageSnapshot) -> WidgetSnapshot<'_> {
    let agents = snapshot
        .agents
        .iter()
        .map(|agent| {
            let windows = snapshot
                .agent_quotas
                .iter()
                .find(|quota| quota.agent == agent.id)
                .map(|quota| {
                    quota
                        .windows
                        .iter()
                        .map(|window| WidgetQuotaWindow {
                            key: &window.key,
                            label: &window.label,
                            available: window.view.available,
                            remaining_percent: window.view.remaining_percent,
                            resets_in_minutes: window.view.resets_in_minutes,
                            stale: window.view.stale,
                            reset_expired: window.view.reset_expired,
                            quality: &window.view.quality,
                        })
                        .collect()
                })
                .unwrap_or_default();
            WidgetAgent {
                id: &agent.id,
                label: agent_label(&agent.id),
                tokens: agent.tokens,
                windows,
            }
        })
        .collect();

    WidgetSnapshot {
        schema_version: 1,
        generated_at: &snapshot.generated_at,
        total_tokens: snapshot.total_tokens,
        agents,
    }
}

fn publisher_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("METRIK_WIDGET_PUBLISHER") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }

    let executable = std::env::current_exe().ok()?;
    let contents = executable.parent().and_then(Path::parent)?;
    let helper = contents.join("Helpers").join("metrik-widget-publish");
    helper.is_file().then_some(helper)
}

fn publish_with_helper(helper: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let mut child = Command::new(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot start WidgetKit publisher {}", helper.display()))?;
    child
        .stdin
        .take()
        .context("WidgetKit publisher stdin is unavailable")?
        .write_all(bytes)
        .context("cannot send the snapshot to the WidgetKit publisher")?;
    let output = child
        .wait_with_output()
        .context("cannot wait for the WidgetKit publisher")?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("WidgetKit publisher failed: {}", detail.trim());
    }
    let path = String::from_utf8(output.stdout)
        .context("WidgetKit publisher returned a non-UTF-8 path")?;
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        anyhow::bail!("WidgetKit publisher returned an empty path");
    }
    Ok(path)
}

fn fallback_directory() -> Result<PathBuf> {
    let base = dirs::data_dir().context("cannot locate the application support directory")?;
    Ok(base.join("Metrik").join("Widget Preview"))
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("widget snapshot has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("cannot create widget group directory {}", parent.display()))?;

    let temporary = parent.join(format!(".{SNAPSHOT_FILE_NAME}.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .with_context(|| format!("cannot create widget snapshot {}", temporary.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("cannot write widget snapshot {}", temporary.display()))?;
    file.sync_all()
        .with_context(|| format!("cannot sync widget snapshot {}", temporary.display()))?;
    drop(file);

    fs::rename(&temporary, path)
        .with_context(|| format!("cannot publish widget snapshot {}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("cannot secure widget snapshot {}", path.display()))?;
    Ok(())
}

pub fn persist(snapshot: &UsageSnapshot) -> Result<PathBuf> {
    let bytes = serde_json::to_vec(&make_payload(snapshot))?;
    let path = if let Some(helper) = publisher_path() {
        publish_with_helper(&helper, &bytes)?
    } else {
        // Cargo tests and unbundled development builds have no signed helper.
        // Keep a readable preview copy without pretending it is an App Group.
        let path = fallback_directory()?.join(SNAPSHOT_FILE_NAME);
        write_atomically(&path, &bytes)?;
        path
    };
    reload_timelines();
    Ok(path)
}

fn reload_timelines() {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(contents) = executable.parent().and_then(Path::parent) else {
        return;
    };
    let helper = contents.join("Helpers").join("metrik-widget-reload");
    if helper.is_file() {
        let _ = std::process::Command::new(helper).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_match_the_public_agent_names() {
        assert_eq!(agent_label("codex"), "ChatGPT");
        assert_eq!(agent_label("zcode"), "GLM");
        assert_eq!(agent_label("opencode"), "OpenCode");
    }
}
