use crate::domain::{sane_resets_at_ms, QuotaSample};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
/// `metrik-statusline.backup.json`，脚本落完额度数据后把 stdin 原样转给
/// 原命令渲染显示；单行输出在行尾追加 5h/7d 百分比，多行输出原样透传
/// （多行 statusLine 自带排版）。卸载时原样恢复备份。
/// 委托命令的执行方式：ps1 脚本走进程内 SetIn 喂 stdin；其他命令（bash/exe
/// 等）走临时文件 + Start-Process 标准输入/输出重定向——PS 5.1 的 `$input|&`
/// 原生管道会随机整段丢输出（真机实测），Start-Process 重定向是唯一稳定
/// 传输；委托挂死时 10 秒超时强杀，不冻结状态栏。
const QUOTA_FILE: &str = "metrik-quota.json";
const BACKUP_FILE: &str = "metrik-statusline.backup.json";
const DELEGATE_PLACEHOLDER: &str = "{{DELEGATE}}";

#[cfg(windows)]
const SCRIPT_FILE: &str = "metrik-statusline.ps1";
#[cfg(not(windows))]
const SCRIPT_FILE: &str = "metrik-statusline.py";

#[cfg(windows)]
const SCRIPT_BODY: &str = r#"# Metrik statusLine hook: persist Claude Code rate limits, no content is stored.
$delegate = '{{DELEGATE}}'
$raw = [Console]::In.ReadToEnd()
try { $data = $raw | ConvertFrom-Json } catch { exit 0 }
$rl = $data.rate_limits
$payload = @{ receivedAtMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
if ($null -ne $rl) {
  $windows = @{}
  foreach ($prop in $rl.PSObject.Properties) {
    $entry = $prop.Value
    if ($null -ne $entry -and $null -ne $entry.used_percentage) {
      $windows[$prop.Name] = @{ usedPercentage = [double]$entry.used_percentage }
      if ($null -ne $entry.resets_at) { $windows[$prop.Name].resetsAt = [double]$entry.resets_at }
    }
  }
  $payload.windows = $windows
}
$target = Join-Path $env:USERPROFILE '.claude\metrik-quota.json'
$tmp = "$target.tmp-$PID"
($payload | ConvertTo-Json -Depth 5 -Compress) | Out-File -FilePath $tmp -Encoding utf8
Move-Item -Force $tmp $target
$quotaParts = @()
if ($payload.windows -and $payload.windows.five_hour) { $quotaParts += ('5h ' + [math]::Round($payload.windows.five_hour.usedPercentage) + '%') }
if ($payload.windows -and $payload.windows.seven_day) { $quotaParts += ('7d ' + [math]::Round($payload.windows.seven_day.usedPercentage) + '%') }
if ($delegate) {
  # 串联模式：显示交给用户原有的 statusLine 命令。单行输出在行尾追加额度；
  # 多行输出原样透传（多行 statusLine 自带排版，追加会把它压坏）。
  # PowerShell 脚本走进程内执行（SetIn 喂 stdin），零编码转换；
  # 其他命令走临时文件 + Start-Process 重定向——PS 5.1 的 $input|& 原生
  # 管道会随机整段丢输出（真机实测），Start-Process 的重定向是唯一稳定传输。
  $lines = @()
  try {
    $psFile = [regex]::Match($delegate, '-File\s+"?([^"]+\.ps1)"?')
    if ($psFile.Success -and (Test-Path $psFile.Groups[1].Value)) {
      [Console]::SetIn((New-Object System.IO.StringReader($raw)))
      $lines = @(& $psFile.Groups[1].Value 2>$null)
    } else {
      # 委托拆成 exe + 参数行："引号 exe + 参数" 或 "裸 exe + 参数"。
      $exe = $null
      $argLine = ""
      $m = [regex]::Match($delegate, '^\s*"([^"]+)"\s*(.*)$')
      if ($m.Success) {
        $exe = $m.Groups[1].Value
        $argLine = $m.Groups[2].Value
      } else {
        $m = [regex]::Match($delegate, '^\s*(\S+)\s*(.*)$')
        if ($m.Success) {
          $exe = $m.Groups[1].Value
          $argLine = $m.Groups[2].Value
        }
      }
      if ($exe -and (Test-Path $exe)) {
        $tmpIn = Join-Path $env:TEMP "metrik-statusline-in-$PID.json"
        $tmpOut = Join-Path $env:TEMP "metrik-statusline-out-$PID.txt"
        try {
          [IO.File]::WriteAllText($tmpIn, $raw)
          try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}
          $proc = Start-Process -FilePath $exe -ArgumentList $argLine `
            -RedirectStandardInput $tmpIn -RedirectStandardOutput $tmpOut `
            -NoNewWindow -PassThru
          # 委托挂死不能冻结状态栏：10 秒超时强杀。
          if (-not $proc.WaitForExit(10000)) { try { $proc.Kill() } catch {} }
          if (Test-Path $tmpOut) { $lines = @(Get-Content $tmpOut -Encoding UTF8) }
        } finally {
          Remove-Item -Force $tmpIn, $tmpOut -ErrorAction SilentlyContinue
        }
      } else {
        # exe 拆不出/不在盘上（PATH 里的裸命令、复合命令）：尽力走旧管道，
        # '& ' 调用运算符必不可少——委托以带引号的 exe 路径开头时，没有它
        # scriptblock 创建会直接抛管道语法错误。
        try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}
        $lines = @($raw | & ([scriptblock]::Create('$input | & ' + $delegate)) 2>$null)
      }
    }
  } catch {}
  $lines = @($lines | ForEach-Object { [string]$_ })
  $out = $lines -join "`n"
  if ($lines.Count -eq 1 -and $out -and $quotaParts) { "$out | " + ($quotaParts -join ' ') }
  elseif ($out) { $out }
  elseif ($quotaParts) { $quotaParts -join ' ' }
} else {
  $model = if ($data.model.display_name) { $data.model.display_name } else { 'Claude' }
  (@($model) + $quotaParts) -join ' | '
}
"#;

#[cfg(not(windows))]
const SCRIPT_BODY: &str = r#"#!/usr/bin/env python3
# Metrik statusLine hook: persist Claude Code rate limits, no content is stored.
import json, os, subprocess, sys, tempfile, time

DELEGATE = "{{DELEGATE}}"

raw = sys.stdin.read()
try:
    data = json.loads(raw)
except Exception:
    sys.exit(0)

payload = {"receivedAtMs": int(time.time() * 1000)}
rl = data.get("rate_limits") or {}
windows = {}
for key, entry in rl.items():
    if isinstance(entry, dict) and entry.get("used_percentage") is not None:
        windows[key] = {"usedPercentage": float(entry["used_percentage"])}
        if entry.get("resets_at") is not None:
            windows[key]["resetsAt"] = float(entry["resets_at"])
if windows:
    payload["windows"] = windows

target = os.path.expanduser("~/.claude/metrik-quota.json")
fd, tmp = tempfile.mkstemp(dir=os.path.dirname(target))
with os.fdopen(fd, "w") as handle:
    json.dump(payload, handle)
os.replace(tmp, target)

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
    model = ((data.get("model") or {}).get("display_name")) or "Claude"
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
    pub fn detected() -> Self {
        Self {
            claude_dir: dirs::home_dir().unwrap_or_default().join(".claude"),
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

    fn read_backup(&self) -> Option<Value> {
        let raw = std::fs::read_to_string(self.backup_path()).ok()?;
        serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()
    }

    /// 把被串联的原命令嵌入脚本。Windows 走 PowerShell 单引号转义；
    /// 其他平台用 JSON 字符串字面量（与 Python 字面量兼容）。
    fn render_script(delegate: &str) -> String {
        #[cfg(windows)]
        {
            SCRIPT_BODY.replace(DELEGATE_PLACEHOLDER, &delegate.replace('\'', "''"))
        }
        #[cfg(not(windows))]
        {
            let quoted = serde_json::to_string(delegate).unwrap_or_else(|_| "\"\"".to_owned());
            SCRIPT_BODY.replace(&format!("\"{DELEGATE_PLACEHOLDER}\""), &quoted)
        }
    }

    /// `render_script` 的逆操作：从已安装的脚本里回读被串联的原命令。
    ///
    /// 备份文件是用户可见的普通文件，会被清理 `.claude`、同步工具或杀软
    /// 删掉。备份没了而 statusLine 仍是我们的时，重装若把 delegate 当成空，
    /// 用户原有的 statusLine 就永久消失（备份已不在，卸载也还原不回来）。
    /// 脚本本身带着 delegate，是这种情况下唯一的真相源。
    fn installed_delegate(&self) -> Option<String> {
        let script = std::fs::read_to_string(self.script_path()).ok()?;
        #[cfg(windows)]
        {
            // 生成时 `'` 转义成 `''`，这里反向还原。
            let line = script
                .lines()
                .find_map(|l| l.strip_prefix("$delegate = '"))?;
            Some(line.strip_suffix('\'')?.replace("''", "'"))
        }
        #[cfg(not(windows))]
        {
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
    fn hook_command(&self) -> String {
        let script = self.script_path();
        if cfg!(windows) {
            let system_root =
                std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
            format!(
                "\"{system_root}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
                script.display()
            )
        } else {
            format!("python3 \"{}\"", script.display())
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
        settings
            .get("statusLine")
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .is_some_and(|command| command.contains(SCRIPT_FILE))
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
        let chained = installed && self.read_backup().is_some();
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

        std::fs::write(self.script_path(), Self::render_script(&delegate))
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
            json!({ "type": "command", "command": self.hook_command(), "padding": 0 }),
        );
        self.write_settings(&settings)?;
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
        if installed_command == self.hook_command() && self.script_path().exists() {
            return Ok(false);
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
    fn install_writes_script_and_status_line_then_uninstall_restores() {
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
        assert!(settings["statusLine"]["command"]
            .as_str()
            .unwrap()
            .contains("metrik-statusline"));
        assert!(hook.script_path().exists());

        let status = hook.uninstall().unwrap();
        assert!(!status.installed);
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["model"], "opus");
        assert!(!hook.script_path().exists());
    }

    /// 回归：statusLine 由 Git Bash 执行，命令必须是带引号的绝对路径。
    /// 裸 `powershell`（PATH 解析不到）和不带引号的绝对路径（反斜杠被
    /// bash 吃掉）都会静默 exit 127，状态栏空白且钩子从不落盘。
    #[cfg(windows)]
    #[test]
    fn windows_hook_command_is_quoted_absolute_path() {
        let test = TestDirectory::new("hookcmd");
        let command = ClaudeHook::with_dir(test.path().to_path_buf()).hook_command();
        assert!(command.starts_with('"'), "exe 路径必须加引号: {command}");
        let exe = command[1..].split('"').next().unwrap();
        assert!(
            exe.to_ascii_lowercase().ends_with("powershell.exe"),
            "第一个 token 应是 powershell.exe: {command}"
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

        // 安装：备份原设置、脚本嵌入原命令、statusLine 指向 Metrik 脚本。
        let status = hook.install().unwrap();
        assert!(status.installed);
        assert!(status.chained);
        let script = fs::read_to_string(hook.script_path()).unwrap();
        assert!(script.contains("my-own-line"));
        assert!(!script.contains(DELEGATE_PLACEHOLDER));
        let backup: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join(BACKUP_FILE)).unwrap())
                .unwrap();
        assert_eq!(backup["command"], "my-own-line 'quoted'");
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(test.path().join("settings.json")).unwrap())
                .unwrap();
        assert!(settings["statusLine"]["command"]
            .as_str()
            .unwrap()
            .contains("metrik-statusline"));

        // 重装不丢串联：statusLine 已是我们的，命令沿用备份。
        let status = hook.install().unwrap();
        assert!(status.chained);
        let script = fs::read_to_string(hook.script_path()).unwrap();
        assert!(script.contains("my-own-line"));

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

    /// 启动自愈：旧版本写坏的命令要修好，串联的原命令不能在修复中丢掉。
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
        assert_eq!(settings["statusLine"]["command"], hook.hook_command());
        let script = fs::read_to_string(hook.script_path()).unwrap();
        assert!(script.contains("my-own-line"), "自愈把 delegate 丢了");

        // 已经正确：不再写盘，否则每次启动都动 settings.json。
        assert!(!hook.repair().unwrap(), "命令正确时不该重写");

        // 命令没问题但脚本被删掉了，状态栏同样是空的，也要补回来。
        fs::remove_file(hook.script_path()).unwrap();
        assert!(hook.repair().unwrap(), "脚本丢失应该被补回");
        assert!(hook.script_path().exists());
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
    fn deleted_backup_recovers_delegate_from_installed_script() {
        let test = TestDirectory::new("lostbackup");
        fs::write(
            test.path().join("settings.json"),
            r#"{"statusLine": {"type": "command", "command": "my-own-line 'quoted'", "padding": 0}}"#,
        )
        .unwrap();
        let hook = ClaudeHook::with_dir(test.path().to_path_buf());
        hook.install().unwrap();

        // 备份没了，但 statusLine 仍指向 Metrik 脚本。
        fs::remove_file(test.path().join(BACKUP_FILE)).unwrap();

        // 重装：delegate 必须从脚本回读，不能退化成无委托。
        hook.install().unwrap();
        let script = fs::read_to_string(hook.script_path()).unwrap();
        assert!(script.contains("my-own-line"), "重装丢了 delegate");

        // 卸载：按脚本里的原命令还原，而不是删掉 statusLine。
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
