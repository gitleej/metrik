//! 用户自己声明的用量来源。
//!
//! 我们没有逐家适配的 Agent，只要它的会话日志是 **Claude 兼容 JSONL**（这个
//! 格式的克隆产品很多——我们的 workbuddy adapter 就是一个解析器同时吃
//! CodeBuddy 与 WorkBuddy），用户在设置里指一下目录就能算进总量，不必等我们
//! 排期。
//!
//! 刻意的边界：
//! - **只认 Claude 兼容 JSONL 一种格式。** 不做"字段映射"式的通用适配——让用户
//!   自己填哪个字段是 input、哪个是 output，等于把最容易出错的口径判断推给他，
//!   而错了不会报错、只会显示一个看着合理的错数字。格式不符就解析不出事件，
//!   如实显示 0，不猜。
//! - **合并进一个 `custom` 槽位。** 各来源的名字在「数据统计」里分别列出，
//!   但图表与占比是合计值。做成动态 Agent 要改动总量、成本、配额、序列全线。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// app_setting 里的键。
const SETTING_KEY: &str = "custom_usage_sources";

/// 上限。声明得再多也是用户自己扛扫描开销，但总要有个数防止配置写崩后
/// 拖垮每次快照。
const MAX_SOURCES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomSource {
    /// 展示名，只用于「数据统计」里区分来源。
    pub name: String,
    /// 会话日志所在目录，递归查找其下的 `*.jsonl`。
    pub path: String,
}

/// 归一化：去空白、丢掉缺名或缺路径的、按路径去重、截到上限。
/// 路径用与项目归类同一套归一化（反斜杠转正斜杠、盘符大写、去尾斜杠），
/// 这样同一个目录写两种形式不会变成两条。
fn normalized(sources: Vec<CustomSource>) -> Vec<CustomSource> {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<CustomSource> = Vec::new();
    for source in sources {
        let name = source.name.trim().to_owned();
        let Some(path) = crate::domain::normalize_project_path(&source.path) else {
            continue;
        };
        if name.is_empty() || seen.contains(&path) {
            continue;
        }
        seen.push(path.clone());
        out.push(CustomSource { name, path });
        if out.len() >= MAX_SOURCES {
            break;
        }
    }
    out
}

pub fn load(connection: &rusqlite::Connection) -> Result<Vec<CustomSource>> {
    let Some(raw) = crate::storage::get_app_setting(connection, SETTING_KEY)? else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    // 配置损坏时当作没声明，不让用量页因此打不开。
    Ok(serde_json::from_str::<Vec<CustomSource>>(&raw)
        .map(normalized)
        .unwrap_or_default())
}

pub fn save(
    connection: &rusqlite::Connection,
    sources: Vec<CustomSource>,
) -> Result<Vec<CustomSource>> {
    let sources = normalized(sources);
    let raw = serde_json::to_string(&sources).context("failed to serialize custom sources")?;
    crate::storage::set_app_setting(connection, SETTING_KEY, &raw)?;
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_ledger() -> rusqlite::Connection {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(include_str!("../migrations/001_init.sql"))
            .unwrap();
        connection
    }

    fn source(name: &str, path: &str) -> CustomSource {
        CustomSource {
            name: name.into(),
            path: path.into(),
        }
    }

    #[test]
    fn round_trips_with_normalization_and_dedup() {
        let connection = memory_ledger();
        assert!(load(&connection).unwrap().is_empty());

        let saved = save(
            &connection,
            vec![
                source("  SomeBuddy  ", "d:\\logs\\somebuddy\\"),
                // 同一目录的另一种写法：去重，保留先写的那条。
                source("重复", "D:/logs/somebuddy"),
                // 缺名或缺路径的丢掉，不留半条配置。
                source("", "D:/logs/x"),
                source("没路径", "   "),
            ],
        )
        .unwrap();

        assert_eq!(saved, vec![source("SomeBuddy", "D:/logs/somebuddy")]);
        assert_eq!(load(&connection).unwrap(), saved);
    }

    #[test]
    fn corrupt_configuration_reads_as_no_sources() {
        let connection = memory_ledger();
        crate::storage::set_app_setting(&connection, SETTING_KEY, "not json").unwrap();
        assert!(load(&connection).unwrap().is_empty());
    }

    #[test]
    fn the_count_is_capped() {
        let many: Vec<CustomSource> = (0..40)
            .map(|index| source(&format!("s{index}"), &format!("D:/logs/{index}")))
            .collect();
        assert_eq!(normalized(many).len(), MAX_SOURCES);
    }
}
