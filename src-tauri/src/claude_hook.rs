use crate::domain::{sane_resets_at_ms, QuotaSample};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;

/// Claude Code 官方配额的零凭据来源：statusLine 钩子。
///
/// Claude Code 每次刷新状态栏都会把当前会话 JSON（含
/// `rate_limits.five_hour / seven_day` 的 used_percentage 与 resets_at）
/// 通过 stdin 推给 statusLine 命令。安装的脚本只提取这两个窗口并原子
/// 写入 `~/.claude/metrik-quota.json`，同时输出一行简洁的状态栏文本；
/// 不读取、不保存对话内容或凭据。
///
/// 用户已有自定义 statusLine 时不覆盖而是串联：原命令备份到
/// `metrik-statusline.backup.json`，钩子落完额度数据后把 stdin 原样转给
/// 原命令渲染显示；单行输出在行尾追加 5h/7d 百分比，多行输出原样透传
/// （多行 statusLine 自带排版）。卸载时原样恢复备份。
/// Windows 由 `metrik.exe --statusline` 直接用管道执行委托，10 秒超时后终止
/// 进程树；非 Windows 仍由生成的 Python 脚本处理。
const QUOTA_FILE: &str = "metrik-quota.json";
const BACKUP_FILE: &str = "metrik-statusline.backup.json";
#[cfg(windows)]
const METADATA_FILE: &str = "metrik-statusline.json";
#[cfg(not(windows))]
const DELEGATE_PLACEHOLDER: &str = "{{DELEGATE}}";
#[cfg(not(windows))]
const QUOTA_PATH_PLACEHOLDER: &str = "{{QUOTA_PATH}}";

#[cfg(windows)]
const SCRIPT_FILE: &str = "metrik-statusline.ps1";
#[cfg(not(windows))]
const SCRIPT_FILE: &str = "metrik-statusline.py";

#[cfg(not(windows))]
const SCRIPT_BODY: &str = r#"#!/usr/bin/env python3
# Metrik statusLine hook: persist Claude Code rate limits, no content is stored.
import json, os, subprocess, sys, tempfile, time

DELEGATE = "{{DELEGATE}}"
# 落盘位置在生成时烘进来，不再从 ~ 现推：设了 CLAUDE_CONFIG_DIR 的用户，
# 配置目录并不在 ~/.claude，现推会写到 Metrik 根本不读的地方。
TARGET = "{{QUOTA_PATH}}"

raw = sys.stdin.read()
# 解析失败不能连累用户：额度抓不到是我们的事，用户原有的 statusLine 不该
# 因此消失。所以这里只记下失败，照样往下走把 stdin 转给委托渲染。
try:
    data = json.loads(raw)
except Exception:
    data = None

windows = {}
if data is not None:
    payload = {"receivedAtMs": int(time.time() * 1000)}
    rl = data.get("rate_limits") or {}
    for key, entry in rl.items():
        if isinstance(entry, dict) and entry.get("used_percentage") is not None:
            windows[key] = {"usedPercentage": float(entry["used_percentage"])}
            if entry.get("resets_at") is not None:
                windows[key]["resetsAt"] = float(entry["resets_at"])
    if windows:
        payload["windows"] = windows
    try:
        fd, tmp = tempfile.mkstemp(dir=os.path.dirname(TARGET))
        try:
            with os.fdopen(fd, "w") as handle:
                json.dump(payload, handle)
            os.replace(tmp, TARGET)
        except Exception:
            # 换名失败就把临时文件带走，别留在用户的配置目录里。
            try:
                os.unlink(tmp)
            except Exception:
                pass
    except Exception:
        pass

quota_parts = []
if "five_hour" in windows:
    quota_parts.append(f"5h {round(windows['five_hour']['usedPercentage'])}%")
if "seven_day" in windows:
    quota_parts.append(f"7d {round(windows['seven_day']['usedPercentage'])}%")

if DELEGATE:
    # 串联模式：显示交给用户原有的 statusLine 命令。单行输出在行尾追加额度；
    # 多行输出原样透传（多行 statusLine 自带排版，追加会把它压坏）。
    out = ""
    try:
        result = subprocess.run(DELEGATE, shell=True, input=raw.encode(), capture_output=True, timeout=10)
        out = result.stdout.decode(errors="replace").rstrip("\n")
    except Exception:
        pass
    if not out.strip():
        out = ""
    lines = out.split("\n") if out else []
    if len(lines) == 1 and quota_parts:
        print(f"{out} | {' '.join(quota_parts)}")
    elif out:
        print(out)
    elif quota_parts:
        print(" ".join(quota_parts))
else:
    model = ((data or {}).get("model") or {}).get("display_name") or "Claude"
    print(" | ".join([model] + quota_parts))
"#;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeHookStatus {
    pub installed: bool,
    /// 已有无法串联的 statusLine（缺 command 字段），安装被拒绝。
    pub conflict: bool,
    /// 已安装且串联了用户原有的 statusLine 命令。
    pub chained: bool,
    pub last_data_at_ms: Option<i64>,
}

#[derive(Deserialize)]
struct QuotaFile {
    #[serde(rename = "receivedAtMs")]
    received_at_ms: i64,
    #[serde(default)]
    windows: std::collections::BTreeMap<String, QuotaWindow>,
}

#[derive(Deserialize)]
struct QuotaWindow {
    #[serde(rename = "usedPercentage")]
    used_percentage: f64,
    #[serde(rename = "resetsAt")]
    resets_at: Option<f64>,
}

#[cfg(windows)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusLineMetadata {
    delegate: String,
    quota_path: PathBuf,
}

#[cfg(windows)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusLinePayload {
    received_at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    windows: Option<std::collections::BTreeMap<String, StatusLineWindow>>,
}

#[cfg(windows)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusLineWindow {
    used_percentage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    resets_at: Option<f64>,
}

#[cfg(windows)]
fn numeric_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse::<f64>().ok())
}

#[cfg(windows)]
fn payload_from_input(input: &Value) -> StatusLinePayload {
    let windows = input
        .get("rate_limits")
        .and_then(Value::as_object)
        .map(|rate_limits| {
            rate_limits
                .iter()
                .filter_map(|(key, entry)| {
                    let used_percentage = numeric_value(entry.get("used_percentage")?)?;
                    let resets_at = entry.get("resets_at").and_then(numeric_value);
                    Some((
                        key.clone(),
                        StatusLineWindow {
                            used_percentage,
                            resets_at,
                        },
                    ))
                })
                .collect()
        });
    StatusLinePayload {
        received_at_ms: chrono::Utc::now().timestamp_millis(),
        windows,
    }
}

#[cfg(windows)]
fn quota_parts(payload: &StatusLinePayload) -> Vec<String> {
    let Some(windows) = payload.windows.as_ref() else {
        return Vec::new();
    };
    [("five_hour", "5h"), ("seven_day", "7d")]
        .into_iter()
        .filter_map(|(key, label)| {
            windows
                .get(key)
                .map(|window| format!("{label} {:.0}%", window.used_percentage))
        })
        .collect()
}

#[cfg(windows)]
fn write_quota_atomically(path: &Path, payload: &StatusLinePayload) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("Claude quota path has no file name")?;
    let staged = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));
    std::fs::write(&staged, serde_json::to_vec(payload)?)
        .context("unable to stage Claude quota snapshot")?;
    let installed = std::fs::rename(&staged, path);
    if installed.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    installed.context("unable to install Claude quota snapshot")
}

#[cfg(windows)]
fn sweep_stale_files(directory: &Path, prefix: &str, stale_before: std::time::SystemTime) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(prefix) {
            continue;
        }
        let is_stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified < stale_before);
        if is_stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(windows)]
fn command_has_shell_syntax(command: &str) -> bool {
    let mut quoted = false;
    command.chars().any(|character| match character {
        '"' => {
            quoted = !quoted;
            false
        }
        '|' | '&' | ';' | '<' | '>' | '(' | ')' if !quoted => true,
        _ => false,
    })
}

#[cfg(windows)]
fn split_delegate_command(command: &str) -> Option<(&str, &str)> {
    let command = command.trim();
    if let Some(quoted) = command.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some((&quoted[..end], quoted[end + 1..].trim_start()));
    }
    let end = command.find(char::is_whitespace).unwrap_or(command.len());
    Some((&command[..end], command[end..].trim_start()))
}

#[cfg(windows)]
fn delegate_process(delegate: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;

    let direct = split_delegate_command(delegate);
    let needs_shell = command_has_shell_syntax(delegate)
        || direct.is_some_and(|(executable, _)| {
            matches!(
                Path::new(executable)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("bat" | "cmd")
            )
        });
    let mut command = if needs_shell {
        let comspec = std::env::var_os("COMSPEC")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let system_root =
                    std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
                PathBuf::from(system_root).join("System32").join("cmd.exe")
            });
        let mut command = std::process::Command::new(comspec);
        command.args(["/D", "/S", "/C"]).raw_arg(delegate);
        command
    } else if let Some((executable, arguments)) = direct {
        let mut command = std::process::Command::new(executable);
        if !arguments.is_empty() {
            command.raw_arg(arguments);
        }
        command
    } else {
        std::process::Command::new(delegate)
    };
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000);
    command
}

#[cfg(windows)]
fn run_delegate(delegate: &str, input: &[u8], timeout: std::time::Duration) -> String {
    use std::io::{Read, Write};

    let Ok(mut child) = delegate_process(delegate).spawn() else {
        return String::new();
    };
    let output = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut output) = output {
            let _ = output.read_to_end(&mut bytes);
        }
        bytes
    });
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input);
    }

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            _ => {
                crate::app_server::terminate_process_tree(&mut child);
                break;
            }
        }
    }
    let bytes = reader.join().unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .trim_end_matches(['\r', '\n'])
        .to_owned()
}

#[cfg(windows)]
fn render_statusline(
    hook: &ClaudeHook,
    input: &[u8],
    temp_dir: &Path,
    delegate_timeout: std::time::Duration,
) -> String {
    let metadata = hook.read_metadata();
    let data = serde_json::from_slice::<Value>(input).ok();
    let payload = data.as_ref().map(payload_from_input);

    if let (Some(metadata), Some(payload)) = (metadata.as_ref(), payload.as_ref()) {
        let _ = write_quota_atomically(&metadata.quota_path, payload);
    }

    let stale_before = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(5 * 60))
        .unwrap_or(std::time::UNIX_EPOCH);
    sweep_stale_files(temp_dir, "metrik-statusline-", stale_before);
    if let Some(quota_dir) = metadata
        .as_ref()
        .and_then(|metadata| metadata.quota_path.parent())
    {
        sweep_stale_files(quota_dir, "metrik-quota.json.tmp-", stale_before);
    }

    let quota_parts = payload.as_ref().map(quota_parts).unwrap_or_default();
    if let Some(delegate) = metadata
        .as_ref()
        .map(|metadata| metadata.delegate.trim())
        .filter(|delegate| !delegate.is_empty())
    {
        let delegated = run_delegate(delegate, input, delegate_timeout);
        if !delegated.is_empty() {
            if !delegated.contains('\r') && !delegated.contains('\n') && !quota_parts.is_empty() {
                return format!("{delegated} | {}", quota_parts.join(" "));
            }
            return delegated;
        }
        return quota_parts.join(" ");
    }

    let model = data
        .as_ref()
        .and_then(|value| value.get("model"))
        .and_then(|value| value.get("display_name"))
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .unwrap_or("Claude");
    if quota_parts.is_empty() {
        model.to_owned()
    } else {
        format!("{model} | {}", quota_parts.join(" "))
    }
}

#[cfg(windows)]
pub fn run_statusline() {
    use std::io::{Read, Write};

    let mut input = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut input);
    let output = render_statusline(
        &ClaudeHook::detected(),
        &input,
        &std::env::temp_dir(),
        std::time::Duration::from_secs(10),
    );
    if !output.is_empty() {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(output.as_bytes());
        let _ = stdout.write_all(b"\n");
    }
}

/// 现有非 Metrik statusLine 的 command 原文（可串联时返回）。
fn foreign_command(settings: &Value) -> Option<String> {
    settings
        .get("statusLine")?
        .get("command")?
        .as_str()
        .filter(|command| !command.trim().is_empty())
        .map(str::to_owned)
}

pub struct ClaudeHook {
    claude_dir: PathBuf,
}

impl ClaudeHook {
    /// Claude 配置目录：优先 `$CLAUDE_CONFIG_DIR`，否则 `~/.claude`。
    ///
    /// 与 `claude_oauth::config_dir` 保持一致。这里原来写死 `~/.claude`：设了
    /// `CLAUDE_CONFIG_DIR` 的用户，钩子会被装进 Claude Code 根本不读的那个
    /// settings.json——界面显示「已安装」，实际永不执行、零数据，还在错误的
    /// 目录里留下脚本和备份。
    pub fn detected() -> Self {
        Self {
            claude_dir: std::env::var_os("CLAUDE_CONFIG_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".claude")),
        }
    }

    #[cfg(test)]
    pub fn with_dir(claude_dir: PathBuf) -> Self {
        Self { claude_dir }
    }

    fn settings_path(&self) -> PathBuf {
        self.claude_dir.join("settings.json")
    }

    fn script_path(&self) -> PathBuf {
        self.claude_dir.join(SCRIPT_FILE)
    }

    fn quota_path(&self) -> PathBuf {
        self.claude_dir.join(QUOTA_FILE)
    }

    fn backup_path(&self) -> PathBuf {
        self.claude_dir.join(BACKUP_FILE)
    }

    #[cfg(windows)]
    fn metadata_path(&self) -> PathBuf {
        self.claude_dir.join(METADATA_FILE)
    }

    fn read_backup(&self) -> Option<Value> {
        let raw = std::fs::read_to_string(self.backup_path()).ok()?;
        serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()
    }

    #[cfg(windows)]
    fn read_metadata(&self) -> Option<StatusLineMetadata> {
        let raw = std::fs::read_to_string(self.metadata_path()).ok()?;
        serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()
    }

    #[cfg(windows)]
    fn expected_metadata(&self, delegate: String) -> Result<StatusLineMetadata> {
        let quota_path = self.quota_path();
        let quota_path = if quota_path.is_absolute() {
            quota_path
        } else {
            std::env::current_dir()
                .context("无法确定 Claude quota 文件的绝对路径")?
                .join(quota_path)
        };
        Ok(StatusLineMetadata {
            delegate,
            quota_path,
        })
    }

    #[cfg(windows)]
    fn write_metadata(&self, metadata: &StatusLineMetadata) -> Result<()> {
        let path = self.metadata_path();
        let staged = path.with_extension(format!("json.metrik-{}", std::process::id()));
        std::fs::write(&staged, serde_json::to_vec_pretty(metadata)?)
            .context("无法写入 Windows statusLine 元数据")?;
        let installed = std::fs::rename(&staged, &path);
        if installed.is_err() {
            let _ = std::fs::remove_file(&staged);
        }
        installed.context("无法安装 Windows statusLine 元数据")
    }

    /// 把被串联的原命令和额度落盘路径嵌入非 Windows 的 Python 脚本。
    #[cfg(not(windows))]
    fn render_script(&self, delegate: &str) -> String {
        let quota = self.quota_path();
        let quota = quota.to_string_lossy();
        let quote =
            |value: &str| serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned());
        SCRIPT_BODY
            .replace(&format!("\"{DELEGATE_PLACEHOLDER}\""), &quote(delegate))
            .replace(&format!("\"{QUOTA_PATH_PLACEHOLDER}\""), &quote(&quota))
    }

    #[cfg(not(windows))]
    fn script_text(&self, delegate: &str) -> String {
        self.render_script(delegate)
    }

    /// 备份文件是用户可见的普通文件，会被清理 `.claude`、同步工具或杀软
    /// 删掉。备份没了而 statusLine 仍是我们的时，重装若把 delegate 当成空，
    /// 用户原有的 statusLine 就永久消失（备份已不在，卸载也还原不回来）。
    /// Windows 原生安装从元数据读取；升级迁移时再退回旧 `.ps1`。其他平台
    /// 仍从生成的 Python 脚本读取。
    fn installed_delegate(&self) -> Option<String> {
        #[cfg(windows)]
        {
            if let Some(metadata) = self.read_metadata() {
                return Some(metadata.delegate);
            }
            let script = std::fs::read_to_string(self.script_path()).ok()?;
            // 生成时 `'` 转义成 `''`，这里反向还原。
            let line = script
                .lines()
                .find_map(|l| l.strip_prefix("$delegate = '"))?;
            Some(line.strip_suffix('\'')?.replace("''", "'"))
        }
        #[cfg(not(windows))]
        {
            let script = std::fs::read_to_string(self.script_path()).ok()?;
            let line = script.lines().find_map(|l| l.strip_prefix("DELEGATE = "))?;
            serde_json::from_str::<String>(line).ok()
        }
    }

    /// Windows 上必须是「带引号的绝对路径」，两个条件缺一不可：
    /// 绝对路径——Path 被写成 REG_SZ 的机器上 `%SystemRoot%` 不展开，
    /// System32 不在任何进程的 PATH 里，裸 `powershell` 解析不到；
    /// 引号——Claude Code 经 Git Bash 执行 statusLine，裸路径的反斜杠
    /// 会被 bash 当转义符吃掉（C:\Windows\... → C:Windows...）。
    /// 两种失败都是 exit 127 + 零输出，状态栏只表现为空白。
    fn hook_command(&self) -> Result<String> {
        #[cfg(windows)]
        {
            let executable = std::env::current_exe().context("无法确定 metrik.exe 的绝对路径")?;
            Ok(format!("\"{}\" --statusline", executable.display()))
        }
        #[cfg(not(windows))]
        {
            Ok(format!("python3 \"{}\"", self.script_path().display()))
        }
    }

    fn read_settings(&self) -> Result<Value> {
        match std::fs::read_to_string(self.settings_path()) {
            Ok(raw) => {
                let trimmed = raw.trim_start_matches('\u{feff}');
                serde_json::from_str(trimmed).context("~/.claude/settings.json 不是有效 JSON")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
            Err(error) => Err(error).context("无法读取 ~/.claude/settings.json"),
        }
    }

    fn write_settings(&self, settings: &Value) -> Result<()> {
        std::fs::create_dir_all(&self.claude_dir)?;
        let path = self.settings_path();
        let staged = path.with_extension(format!("json.metrik-{}", std::process::id()));
        std::fs::write(&staged, serde_json::to_string_pretty(settings)?)?;
        let installed = std::fs::rename(&staged, &path);
        if installed.is_err() {
            let _ = std::fs::remove_file(&staged);
        }
        installed.context("无法更新 ~/.claude/settings.json")
    }

    fn status_line_is_ours(&self, settings: &Value) -> bool {
        let Some(command) = settings
            .get("statusLine")
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
        else {
            return false;
        };
        if command.contains(SCRIPT_FILE) {
            return true;
        }
        #[cfg(windows)]
        {
            if self
                .hook_command()
                .is_ok_and(|expected| command == expected)
            {
                return true;
            }
            let executable = split_delegate_command(command).map(|(executable, _)| executable);
            command.trim_end().ends_with("--statusline")
                && executable.is_some_and(|executable| {
                    Path::new(executable)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|name| name.eq_ignore_ascii_case("metrik.exe"))
                })
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    pub fn status(&self) -> Result<ClaudeHookStatus> {
        let settings = self.read_settings()?;
        let installed = self.status_line_is_ours(&settings);
        // 只有存在且无法串联（缺 command 字段）的 statusLine 才算冲突。
        let conflict = !installed
            && settings
                .get("statusLine")
                .is_some_and(|value| !value.is_null())
            && foreign_command(&settings).is_none();
        let chained = installed
            && (self.read_backup().is_some()
                || self
                    .installed_delegate()
                    .is_some_and(|value| !value.is_empty()));
        let last_data_at_ms = self.read_quota_file().map(|file| file.received_at_ms);
        Ok(ClaudeHookStatus {
            installed,
            conflict,
            chained,
            last_data_at_ms,
        })
    }

    pub fn install(&self) -> Result<ClaudeHookStatus> {
        let mut settings = self.read_settings()?;
        std::fs::create_dir_all(&self.claude_dir)?;

        // 已有他人 statusLine：备份原设置并串联其 command；重装时沿用已备份的命令。
        let mut delegate = String::new();
        if self.status_line_is_ours(&settings) {
            delegate = self
                .read_backup()
                .as_ref()
                .and_then(|backup| backup.get("command"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                // 备份被删而脚本还在：从脚本回读。否则这里会把 delegate 当成
                // 空的重装成「无委托」，用户原有的 statusLine 就永久没了。
                .or_else(|| self.installed_delegate())
                .unwrap_or_default();
        } else if let Some(existing) = settings
            .get("statusLine")
            .filter(|value| !value.is_null())
            .cloned()
        {
            let Some(command) = foreign_command(&settings) else {
                bail!(
                    "Claude Code 已配置无法串联的 statusLine（缺少 command 字段），为避免覆盖未安装。"
                );
            };
            std::fs::write(self.backup_path(), serde_json::to_string_pretty(&existing)?)
                .context("无法备份原有 statusLine 设置")?;
            delegate = command;
        }

        #[cfg(windows)]
        self.write_metadata(&self.expected_metadata(delegate.clone())?)?;

        #[cfg(not(windows))]
        std::fs::write(self.script_path(), self.script_text(&delegate))
            .context("无法写入钩子脚本")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                self.script_path(),
                std::fs::Permissions::from_mode(0o755),
            );
        }

        let root = settings
            .as_object_mut()
            .context("settings.json 顶层不是对象")?;
        root.insert(
            "statusLine".into(),
            json!({ "type": "command", "command": self.hook_command()?, "padding": 0 }),
        );
        self.write_settings(&settings)?;
        #[cfg(windows)]
        let _ = std::fs::remove_file(self.script_path());
        self.status()
    }

    /// 启动时自愈：statusLine 已经属于 Metrik，但命令过时或脚本不在了，重装一次。
    ///
    /// `install()` 只有界面上那个开关会调，升级和开机都不会。所以旧版本写坏的
    /// statusLine 升级之后依然是坏的：用户看到空白状态栏，而 Metrik 界面显示
    /// 「已安装」，没有任何线索提示他去关一次开关再打开。修一版生成器救不了
    /// 已经写坏的那批人，得在启动时回头补一次。
    ///
    /// 两条边界：statusLine 不属于 Metrik 时一律不碰，这是别人的配置；命令已经
    /// 正确且脚本在位时不写盘，否则每次启动都动 settings.json。
    pub fn repair(&self) -> Result<bool> {
        let settings = self.read_settings()?;
        if !self.status_line_is_ours(&settings) {
            return Ok(false);
        }
        let installed_command = settings
            .get("statusLine")
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        #[cfg(windows)]
        {
            let delegate = self.installed_delegate().unwrap_or_default();
            let expected_metadata = self.expected_metadata(delegate)?;
            if installed_command == self.hook_command()?
                && self.read_metadata().as_ref() == Some(&expected_metadata)
                && !self.script_path().exists()
            {
                return Ok(false);
            }
        }
        #[cfg(not(windows))]
        {
            // 脚本正文一并比对，不能只看文件在不在：命令从外面看是对的，脚本
            // 正文却可能是旧版本生成的。拿回读出来的 delegate 重新渲染一遍，
            // 以后改 SCRIPT_BODY 也能自动补上。
            let installed_script = std::fs::read_to_string(self.script_path()).unwrap_or_default();
            let expected_script = self.script_text(&self.installed_delegate().unwrap_or_default());
            if installed_command == self.hook_command()? && installed_script == expected_script {
                return Ok(false);
            }
        }
        self.install()?;
        Ok(true)
    }

    pub fn uninstall(&self) -> Result<ClaudeHookStatus> {
        let mut settings = self.read_settings()?;
        if self.status_line_is_ours(&settings) {
            let root = settings
                .as_object_mut()
                .context("settings.json 顶层不是对象")?;
            // 串联安装的：把用户原有的 statusLine 原样恢复。
            // 备份被删时退而求其次，用脚本里的原命令重建——宁可丢 padding
            // 之类的次要字段，也不能把用户的 statusLine 整个删掉。
            let restored = self.read_backup().or_else(|| {
                let delegate = self.installed_delegate().filter(|d| !d.is_empty())?;
                Some(json!({ "type": "command", "command": delegate, "padding": 0 }))
            });
            match restored {
                Some(backup) => {
                    root.insert("statusLine".into(), backup);
                }
                None => {
                    root.remove("statusLine");
                }
            }
            self.write_settings(&settings)?;
        }
        let _ = std::fs::remove_file(self.script_path());
        #[cfg(windows)]
        let _ = std::fs::remove_file(self.metadata_path());
        let _ = std::fs::remove_file(self.quota_path());
        let _ = std::fs::remove_file(self.backup_path());
        self.status()
    }

    fn read_quota_file(&self) -> Option<QuotaFile> {
        let raw = std::fs::read_to_string(self.quota_path()).ok()?;
        serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()
    }

    /// 把钩子落地的全部官方窗口转换成 QuotaSample（原始窗口名作 key）；
    /// 文件缺失或格式异常返回空，不猜测、不沿用陈旧文件之外的任何来源。
    pub fn quota_samples(&self) -> Vec<QuotaSample> {
        let Some(file) = self.read_quota_file() else {
            return Vec::new();
        };
        file.windows
            .iter()
            .map(|(key, window)| QuotaSample {
                adapter_id: "claude",
                window_key: key.clone(),
                remaining_percent: (100.0 - window.used_percentage).clamp(0.0, 100.0),
                resets_at_ms: window
                    .resets_at
                    .map(|value| (value * 1000.0) as i64)
                    .and_then(|value| sane_resets_at_ms(key, value, file.received_at_ms)),
                collected_at_ms: file.received_at_ms,
                source_label: "statusLine 钩子".into(),
                quality: "official_snapshot",
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "metrik-claude-hook-{label}-{}-{}",
                std::process::id(),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn install_writes_hook_and_status_line_then_uninstall_restores() {
        let test = TestDirectory::new("roundtrip");
        fs::write(
            test.path().join("settings.json"),
            r#"{"model": "opus", "env": {"KEY": "value"}}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());

        let status = hook.install().unwrap();
        assert!(status.installed);
        assert!(!status.conflict);
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["model"], "opus");
        assert_eq!(settings["env"]["KEY"], "value");
        #[cfg(windows)]
        {
            assert!(settings["statusLine"]["command"]
                .as_str()
                .unwrap()
                .ends_with(" --statusline"));
            assert!(hook.metadata_path().exists());
            assert!(!hook.script_path().exists());
        }
        #[cfg(not(windows))]
        {
            assert!(settings["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains("metrik-statusline"));
            assert!(hook.script_path().exists());
        }

        let status = hook.uninstall().unwrap();
        assert!(!status.installed);
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["model"], "opus");
        assert!(!hook.script_path().exists());
        #[cfg(windows)]
        assert!(!hook.metadata_path().exists());
    }

    /// 回归：statusLine 由 Git Bash 执行，命令必须是带引号的绝对路径。
    /// 裸 `powershell`（PATH 解析不到）和不带引号的绝对路径（反斜杠被
    /// bash 吃掉）都会静默 exit 127，状态栏空白且钩子从不落盘。
    #[cfg(windows)]
    #[test]
    fn windows_hook_command_is_quoted_absolute_path() {
        let test = TestDirectory::new("hookcmd");
        let command = ClaudeHook::with_dir(test.path().to_path_buf())
            .hook_command()
            .unwrap();
        assert!(command.starts_with('"'), "exe 路径必须加引号: {command}");
        let exe = command[1..].split('"').next().unwrap();
        assert!(
            command.ends_with("\" --statusline"),
            "第一个 token 后只能是原生模式参数: {command}"
        );
        assert!(
            std::path::Path::new(exe).is_absolute(),
            "必须用绝对路径，不能依赖 PATH: {command}"
        );
    }

    #[test]
    fn existing_foreign_status_line_is_chained_and_restored() {
        let test = TestDirectory::new("chain");
        fs::write(
            test.path().join("settings.json"),
            r#"{"statusLine": {"type": "command", "command": "my-own-line 'quoted'", "padding": 0}}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());

        // 可串联的 statusLine 不算冲突。
        let status = hook.status().unwrap();
        assert!(!status.conflict);
        assert!(!status.chained);

        // 安装：备份原设置、持久化原命令、statusLine 指向 Metrik。
        let status = hook.install().unwrap();
        assert!(status.installed);
        assert!(status.chained);
        #[cfg(windows)]
        assert_eq!(
            hook.read_metadata().unwrap().delegate,
            "my-own-line 'quoted'"
        );
        #[cfg(not(windows))]
        {
            let script = fs::read_to_string(hook.script_path()).unwrap();
            assert!(script.contains("my-own-line"));
            assert!(!script.contains(DELEGATE_PLACEHOLDER));
        }
        let backup: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join(BACKUP_FILE)).unwrap())
                .unwrap();
        assert_eq!(backup["command"], "my-own-line 'quoted'");
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            settings["statusLine"]["command"],
            hook.hook_command().unwrap()
        );

        // 重装不丢串联：statusLine 已是我们的，命令沿用备份。
        let status = hook.install().unwrap();
        assert!(status.chained);
        assert_eq!(hook.installed_delegate().unwrap(), "my-own-line 'quoted'");

        // 卸载：原有 statusLine 原样恢复，备份清理。
        let status = hook.uninstall().unwrap();
        assert!(!status.installed);
        assert!(!status.chained);
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["statusLine"]["command"], "my-own-line 'quoted'");
        assert_eq!(settings["statusLine"]["padding"], 0);
        assert!(!test.path().join(BACKUP_FILE).exists());
    }

    /// 在隔离环境里运行 Windows 原生 statusLine 逻辑。
    #[cfg(windows)]
    fn run_native(hook: &ClaudeHook, temp: &Path, payload: &[u8]) -> String {
        render_statusline(hook, payload, temp, std::time::Duration::from_secs(10))
    }

    /// 回归：额度落盘位置必须是安装元数据保存的绝对路径。
    ///
    /// 不能从运行时的 USERPROFILE 现推，否则设置了 CLAUDE_CONFIG_DIR 的用户
    /// 会显示「已安装」却永远没有数据。
    #[cfg(windows)]
    #[test]
    fn windows_native_writes_quota_to_the_installed_absolute_path() {
        let test = TestDirectory::new("quotapath");
        let temp = test.path().join("temp");
        fs::create_dir_all(&temp).unwrap();
        let hook = ClaudeHook::with_dir(test.path().join("cfg"));
        fs::create_dir_all(test.path().join("cfg")).unwrap();
        hook.install().unwrap();

        let mut metadata = hook.read_metadata().unwrap();
        assert!(metadata.quota_path.is_absolute());
        assert_eq!(metadata.quota_path, hook.quota_path());
        let installed_quota_dir = test.path().join("installed-quota");
        fs::create_dir_all(&installed_quota_dir).unwrap();
        metadata.quota_path = installed_quota_dir.join(QUOTA_FILE);
        hook.write_metadata(&metadata).unwrap();
        let output = run_native(
            &hook,
            &temp,
            br#"{"model":{"display_name":"Sonnet"},"rate_limits":{"five_hour":{"used_percentage":42},"seven_day":{"used_percentage":19.6,"resets_at":1785197748}}}"#,
        );
        assert_eq!(output, "Sonnet | 5h 42% 7d 20%");

        let written =
            fs::read_to_string(&metadata.quota_path).expect("额度必须写进安装时确定的目录");
        let quota: Value = serde_json::from_str(&written).unwrap();
        assert_eq!(quota["windows"]["five_hour"]["usedPercentage"], 42.0);
        assert_eq!(quota["windows"]["seven_day"]["resetsAt"], 1785197748.0);
        assert!(
            !hook.quota_path().exists(),
            "不能从运行时 hook 目录重推落盘路径"
        );
        assert!(!metadata
            .quota_path
            .with_file_name(format!("{QUOTA_FILE}.tmp-{}", std::process::id()))
            .exists());
    }

    /// 回归：负载解析失败时，用户原有的 statusLine 仍然要渲染。
    ///
    /// 原来是 `catch { exit 0 }`——我们自己解析不了就整个退出，把用户的
    /// statusLine 一起带走了。额度抓不到是我们的事，不该由用户承担。
    #[cfg(windows)]
    #[test]
    fn windows_native_forwards_unparsable_input_to_delegate() {
        let test = TestDirectory::new("badjson");
        let temp = test.path().join("temp");
        fs::create_dir_all(&temp).unwrap();
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        fs::write(
            test.path().join("settings.json"),
            json!({
                "statusLine": {
                    "type": "command",
                    "command": format!(r#""{system_root}\System32\cmd.exe" /D /C findstr /R ".*""#),
                }
            })
            .to_string(),
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        hook.install().unwrap();

        let out = run_native(&hook, &temp, b"this is not json at all");
        assert_eq!(out, "this is not json at all");
        assert!(!hook.quota_path().exists(), "无效 JSON 不应写入额度文件");
    }

    /// 单行委托追加额度；多行面板保持自己的排版，不能被压成一行。
    #[cfg(windows)]
    #[test]
    fn windows_native_preserves_multiline_delegate_layout() {
        let test = TestDirectory::new("multiline");
        let temp = test.path().join("temp");
        fs::create_dir_all(&temp).unwrap();
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        let settings = |command: String| {
            json!({
                "statusLine": {
                    "type": "command",
                    "command": command,
                }
            })
            .to_string()
        };
        let payload = br#"{"rate_limits":{"five_hour":{"used_percentage":7}}}"#;

        fs::write(
            test.path().join("settings.json"),
            settings(format!(
                r#""{system_root}\System32\cmd.exe" /D /C echo ONE"#
            )),
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        hook.install().unwrap();
        assert_eq!(run_native(&hook, &temp, payload), "ONE | 5h 7%");

        let multiline_script = test.path().join("multiline.cmd");
        fs::write(
            &multiline_script,
            "@echo off\r\necho FIRST\r\necho SECOND\r\n",
        )
        .unwrap();
        let multiline = format!(r#""{}""#, multiline_script.display());
        let mut metadata = hook.read_metadata().unwrap();
        metadata.delegate = multiline;
        hook.write_metadata(&metadata).unwrap();
        assert_eq!(run_native(&hook, &temp, payload), "FIRST\r\nSECOND");
    }

    /// 回归：原生路径必须清理上一次调用被终止后留下的旧临时文件。
    ///
    /// Claude Code 每次重渲染状态栏都会终止上一次调用，而串联一个委托要好几秒，
    /// 所以绝大多数调用都是中途被终止的，`finally` 没有机会执行，临时文件永远留在
    /// 盘上（真机实测 50 分钟残留 124 个）。
    #[cfg(windows)]
    #[test]
    fn windows_native_sweeps_temp_files_from_terminated_runs() {
        use std::time::{Duration, SystemTime};

        let test = TestDirectory::new("tempsweep");
        let home = test.path().join("home");
        let temp = test.path().join("temp");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(&temp).unwrap();
        let hook = ClaudeHook::with_dir(home.join(".claude"));
        hook.install().unwrap();

        // 上一次被终止后留下的残留，以及一份正在被别的调用使用的新文件。
        let orphans = [
            temp.join("metrik-statusline-in-999999.json"),
            temp.join("metrik-statusline-out-999999.txt"),
            home.join(".claude").join("metrik-quota.json.tmp-999999"),
        ];
        let live = temp.join("metrik-statusline-in-111111.json");
        for path in orphans.iter().chain([&live]) {
            fs::write(path, "{}").unwrap();
        }
        let long_ago = SystemTime::now() - Duration::from_secs(30 * 60);
        for path in &orphans {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(long_ago)
                .unwrap();
        }

        run_native(
            &hook,
            &temp,
            br#"{"rate_limits":{"five_hour":{"used_percentage":7}}}"#,
        );

        for path in &orphans {
            assert!(!path.exists(), "过期的临时文件没被清掉: {}", path.display());
        }
        assert!(live.exists(), "还在用的临时文件不能被误删");
    }

    /// 回归：委托超过时限时要终止整个进程树，不能冻结状态栏或留下后代进程。
    #[cfg(windows)]
    #[test]
    fn windows_native_times_out_and_terminates_delegate_tree() {
        use std::os::windows::process::CommandExt;
        use std::time::{Duration, Instant};

        let test = TestDirectory::new("timeout");
        let temp = test.path().join("temp");
        fs::create_dir_all(&temp).unwrap();
        let marker = test.path().join("descendant.txt");
        let script = test.path().join("delegate.ps1");
        fs::write(
            &script,
            r#"$child = Start-Process -FilePath "$env:SystemRoot\System32\PING.EXE" -ArgumentList "-n","12","127.0.0.1" -WindowStyle Hidden -PassThru
$child.Id | Set-Content -LiteralPath $args[0] -Encoding ascii
Wait-Process -Id $child.Id
"#,
        )
        .unwrap();
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        fs::write(
            test.path().join("settings.json"),
            json!({
                "statusLine": {
                    "type": "command",
                    "command": format!(
                        r#""{system_root}\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "{}" "{}""#,
                        script.display(),
                        marker.display()
                    ),
                }
            })
            .to_string(),
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        hook.install().unwrap();

        let started = Instant::now();
        let output = render_statusline(&hook, b"{}", &temp, Duration::from_secs(2));
        assert_eq!(output, "");
        assert!(started.elapsed() < Duration::from_secs(6));

        let descendant_pid = fs::read_to_string(&marker)
            .expect("delegate should record its descendant before timing out")
            .trim()
            .parse::<u32>()
            .unwrap();
        let listing = std::process::Command::new("tasklist.exe")
            .args([
                "/FI",
                &format!("PID eq {descendant_pid}"),
                "/FO",
                "CSV",
                "/NH",
            ])
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        let descendant_alive =
            String::from_utf8_lossy(&listing.stdout).contains(&format!("\"{descendant_pid}\""));
        if descendant_alive {
            let _ = std::process::Command::new("taskkill.exe")
                .args(["/PID", &descendant_pid.to_string(), "/T", "/F"])
                .creation_flags(0x0800_0000)
                .status();
        }
        assert!(!descendant_alive, "超时后代进程仍在运行: {descendant_pid}");
    }

    /// Windows 升级自愈：旧 `.ps1` 仍是第二真相源时，把 delegate 迁入元数据，
    /// 改写 settings.json 指向原生入口，并删除旧脚本。
    #[cfg(windows)]
    #[test]
    fn repair_migrates_legacy_windows_script_to_native_metadata() {
        let test = TestDirectory::new("migratelegacy");
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        fs::write(
            hook.script_path(),
            "\u{feff}# legacy hook\r\n$delegate = 'my-own-line ''quoted'''\r\n",
        )
        .unwrap();
        fs::write(
            test.path().join("settings.json"),
            json!({
                "statusLine": {
                    "type": "command",
                    "command": format!(
                        r#"powershell.exe -NoProfile -File "{}""#,
                        hook.script_path().display()
                    ),
                    "padding": 0,
                }
            })
            .to_string(),
        )
        .unwrap();

        assert!(hook.repair().unwrap());
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            settings["statusLine"]["command"],
            hook.hook_command().unwrap()
        );
        let metadata = hook.read_metadata().unwrap();
        assert_eq!(metadata.delegate, "my-own-line 'quoted'");
        assert!(metadata.quota_path.is_absolute());
        assert!(!hook.script_path().exists());
        assert!(!hook.repair().unwrap(), "迁移完成后不应每次启动都重写");
    }

    /// 启动自愈：旧版本写坏的命令要修好，串联的原命令不能在修复中丢掉。
    #[cfg(not(windows))]
    #[test]
    fn repair_rewrites_a_stale_command_and_keeps_the_delegate() {
        let test = TestDirectory::new("repairstale");
        fs::write(
            test.path().join("settings.json"),
            r#"{"statusLine": {"type": "command", "command": "my-own-line", "padding": 0}}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        hook.install().unwrap();

        // 模拟旧版本留下的坏命令：仍指向 Metrik 脚本，但不是当前生成器的产物。
        let stale = format!("powershell -File {}", hook.script_path().display());
        fs::write(
            test.path().join("settings.json"),
            format!(
                r#"{{"statusLine": {{"type": "command", "command": {}, "padding": 0}}}}"#,
                serde_json::to_string(&stale).unwrap()
            ),
        )
        .unwrap();

        assert!(hook.repair().unwrap(), "过时的命令应该被修复");
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            settings["statusLine"]["command"],
            hook.hook_command().unwrap()
        );
        let script = fs::read_to_string(hook.script_path()).unwrap();
        assert!(script.contains("my-own-line"), "自愈把 delegate 丢了");

        // 已经正确：不再写盘，否则每次启动都动 settings.json。
        assert!(!hook.repair().unwrap(), "命令正确时不该重写");

        // 命令没问题但脚本被删掉了，状态栏同样是空的，也要补回来。
        fs::remove_file(hook.script_path()).unwrap();
        assert!(hook.repair().unwrap(), "脚本丢失应该被补回");
        assert!(hook.script_path().exists());

        // 命令没问题、脚本也在，但脚本正文是旧版本生成的：从外面看不出问题，
        // 串联却可能已经静默失效（Windows 上缺 BOM 就是这种情况）。
        // 这里在尾部追加一行来模拟「正文与当前生成器不一致」，不碰 delegate 那行，
        // 也不依赖任何平台特有的差异——按平台构造过时正文会让这条断言在另一个
        // 平台上变成空操作，CI 才抓到过一次。
        let stale_script = format!(
            "{}\n# 旧版本留下的正文\n",
            fs::read_to_string(hook.script_path()).unwrap()
        );
        fs::write(hook.script_path(), &stale_script).unwrap();
        assert!(hook.repair().unwrap(), "过时的脚本正文应该被重新生成");
        assert_eq!(
            fs::read_to_string(hook.script_path()).unwrap(),
            hook.script_text("my-own-line"),
            "重新生成的脚本应与当前生成器一致"
        );
        assert!(!hook.repair().unwrap(), "重新生成之后不该再写盘");
    }

    /// 自愈只修 Metrik 自己的 statusLine。别人的配置——包括用户压根没装钩子
    /// 的情况——一律不碰，否则自愈本身就变成了这次要修的那种覆盖。
    #[test]
    fn repair_never_touches_a_foreign_or_absent_status_line() {
        let test = TestDirectory::new("repairforeign");
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());

        // 没有 settings.json：不该无中生有装一个钩子。
        assert!(!hook.repair().unwrap());
        assert!(!test.path().join("settings.json").exists());

        // 别人的 statusLine：原样保留。
        let foreign = r#"{"statusLine": {"type": "command", "command": "my-own-line"}}"#;
        fs::write(test.path().join("settings.json"), foreign).unwrap();
        assert!(!hook.repair().unwrap());
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["statusLine"]["command"], "my-own-line");
        assert!(!hook.script_path().exists());
    }

    /// 回归：备份文件被删掉（用户清理 .claude、同步工具、杀软）之后，
    /// 重装不能把用户原有的 statusLine 静默降级成「无委托」，卸载也不能
    /// 把 statusLine 整个删掉——两者都会让原配置永久消失。
    #[test]
    fn deleted_backup_recovers_delegate_from_installation_state() {
        let test = TestDirectory::new("lostbackup");
        fs::write(
            test.path().join("settings.json"),
            r#"{"statusLine": {"type": "command", "command": "my-own-line 'quoted'", "padding": 0}}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        hook.install().unwrap();

        // 备份没了，但 statusLine 仍指向 Metrik。
        fs::remove_file(test.path().join(BACKUP_FILE)).unwrap();

        // 重装：delegate 必须从安装状态回读，不能退化成无委托。
        hook.install().unwrap();
        assert_eq!(
            hook.installed_delegate().unwrap(),
            "my-own-line 'quoted'",
            "重装丢了 delegate"
        );

        // 卸载：按持久化的原命令还原，而不是删掉 statusLine。
        hook.uninstall().unwrap();
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["statusLine"]["command"], "my-own-line 'quoted'");
    }

    #[test]
    fn status_line_without_command_field_is_a_conflict() {
        let test = TestDirectory::new("conflict");
        fs::write(
            test.path().join("settings.json"),
            r#"{"statusLine": {"type": "something-else"}}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());

        let status = hook.status().unwrap();
        assert!(status.conflict);
        let error = hook.install().unwrap_err();
        assert!(error.to_string().contains("无法串联"));
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["statusLine"]["type"], "something-else");
    }

    #[test]
    fn quota_file_converts_to_remaining_percent_samples() {
        let test = TestDirectory::new("quota");
        fs::write(
            test.path().join(QUOTA_FILE),
            r#"{"receivedAtMs": 1783000000000,
                "windows": {
                    "five_hour": {"usedPercentage": 6.0, "resetsAt": 1783003600.5},
                    "seven_day": {"usedPercentage": 41.5}
                }}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());

        let samples = hook.quota_samples();

        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].adapter_id, "claude");
        assert_eq!(samples[0].window_key, "five_hour");
        assert!((samples[0].remaining_percent - 94.0).abs() < f64::EPSILON);
        assert_eq!(samples[0].resets_at_ms, Some(1_783_003_600_500));
        assert_eq!(samples[1].window_key, "seven_day");
        assert!((samples[1].remaining_percent - 58.5).abs() < f64::EPSILON);
        assert_eq!(samples[1].resets_at_ms, None);

        // 缺失文件 → 空，不猜测。
        let empty = ClaudeHook::with_dir(test.path().join("missing"));
        assert!(empty.quota_samples().is_empty());
    }

    #[test]
    fn sentinel_resets_at_is_dropped_but_plausible_kept() {
        let test = TestDirectory::new("sentinel");
        // Claude Code 在重置时间未知时下发哨兵值（1900000000 秒 ≈ 2030 年），
        // 展示出来是"1331 天后重置"（用户实测）。超出窗口语义的重置时间丢弃。
        fs::write(
            test.path().join(QUOTA_FILE),
            r#"{"receivedAtMs": 1784938548385,
                "windows": {
                    "five_hour": {"usedPercentage": 42.3, "resetsAt": 1900000000},
                    "seven_day": {"usedPercentage": 18.7, "resetsAt": 1900000000}
                }}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        let samples = hook.quota_samples();
        assert_eq!(samples.len(), 2);
        assert!(samples.iter().all(|sample| sample.resets_at_ms.is_none()));

        // 窗口语义内的重置时间保留：5h 窗 +2 小时、7d 窗 +3 天。
        fs::write(
            test.path().join(QUOTA_FILE),
            r#"{"receivedAtMs": 1784938548385,
                "windows": {
                    "five_hour": {"usedPercentage": 42.3, "resetsAt": 1784945748.0},
                    "seven_day": {"usedPercentage": 18.7, "resetsAt": 1785197748.0}
                }}"#,
        )
        .unwrap();
        let samples = hook.quota_samples();
        assert_eq!(samples[0].resets_at_ms, Some(1_784_945_748_000));
        assert_eq!(samples[1].resets_at_ms, Some(1_785_197_748_000));
    }
}
