//! 自定义来源的端到端验收：声明一个目录 → 扫描 → 事件确实以 `custom` 落账。
//!
//! 用真实的 Claude 兼容 JSONL 做样本（本机 `~/.claude/projects` 下就是这个格式），
//! 验证「用户指一下目录就能算进总量」这条路真的通，而不是只在单测里通。

#[cfg(test)]
mod tests {
    use crate::custom_sources::{self, CustomSource};
    use crate::domain::AGENT_IDS;
    use std::io::Write;

    /// 一条 Claude 兼容 JSONL 记录：格式与 `~/.claude/projects/**/*.jsonl` 相同。
    /// 时间取当前之前的若干分钟——写死的时刻换算成本地时间后可能落在"现在"
    /// 之后，会被周期窗口的上界挡掉。
    fn message_line(message_id: &str, minutes_ago: i64, input: i64, output: i64) -> String {
        let timestamp = (chrono::Utc::now() - chrono::Duration::minutes(minutes_ago))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        format!(
            r#"{{"type":"assistant","timestamp":"{timestamp}","sessionId":"sess-1","cwd":"D:\\work\\demo","message":{{"id":"{message_id}","model":"some-model-v1","usage":{{"input_tokens":{input},"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":{output}}}}}}}"#
        )
    }

    #[test]
    fn a_declared_directory_lands_in_the_custom_slot() {
        let dir = std::env::temp_dir().join(format!(
            "metrik-custom-e2e-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("session.jsonl");
        let mut file = std::fs::File::create(&log).unwrap();
        writeln!(file, "{}", message_line("msg-a", 120, 100, 20)).unwrap();
        writeln!(file, "{}", message_line("msg-b", 60, 50, 5)).unwrap();
        drop(file);

        let database = std::env::temp_dir().join(format!(
            "metrik-custom-e2e-{}-{}.sqlite3",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));

        {
            let connection = crate::storage::open_database(&database).unwrap();
            custom_sources::save(
                &connection,
                vec![CustomSource {
                    name: "演示来源".into(),
                    path: dir.to_string_lossy().into_owned(),
                }],
            )
            .unwrap();
        }

        let quota_cache = std::sync::Mutex::new(std::collections::HashMap::new());
        let snapshot =
            crate::engine::build_snapshot(&database, "month", &quota_cache, false).unwrap();

        let custom = snapshot
            .agents
            .iter()
            .find(|agent| agent.id == "custom")
            .expect("custom 必须是一个可见 Agent");
        // 两条消息合计 175：(100+20) + (50+5)。
        assert_eq!(custom.tokens, 175, "自定义来源的用量没有计入 custom 槽位");
        assert!(custom.detected, "声明了来源就该算检测到");

        // 声明的来源名要在「数据统计」里列出来，否则用户看不到分项。
        let view = snapshot
            .sources
            .iter()
            .find(|source| source.id == "custom-local")
            .expect("数据统计里必须有自定义来源这一条");
        assert!(
            view.detail.contains("演示来源"),
            "来源名没有列出：{}",
            view.detail
        );
        assert_eq!(view.quality_label, "精确解析");

        assert!(AGENT_IDS.contains(&"custom"));

        std::fs::remove_file(database).ok();
        std::fs::remove_dir_all(dir).ok();
    }

    /// 格式不符的目录不能编数字：解析不出事件就如实是 0，且不该把来源标成出错
    /// （用户可能只是指错了目录，不是我们读坏了）。
    #[test]
    fn a_directory_in_another_format_yields_zero_rather_than_guesses() {
        let dir = std::env::temp_dir().join(format!(
            "metrik-custom-e2e-bad-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut file = std::fs::File::create(dir.join("other.jsonl")).unwrap();
        writeln!(file, r#"{{"totally":"different","tokens":12345}}"#).unwrap();
        drop(file);

        let database = std::env::temp_dir().join(format!(
            "metrik-custom-e2e-bad-{}-{}.sqlite3",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        {
            let connection = crate::storage::open_database(&database).unwrap();
            custom_sources::save(
                &connection,
                vec![CustomSource {
                    name: "格式不符".into(),
                    path: dir.to_string_lossy().into_owned(),
                }],
            )
            .unwrap();
        }

        let quota_cache = std::sync::Mutex::new(std::collections::HashMap::new());
        let snapshot =
            crate::engine::build_snapshot(&database, "month", &quota_cache, false).unwrap();
        let custom = snapshot
            .agents
            .iter()
            .find(|agent| agent.id == "custom")
            .unwrap();
        assert_eq!(custom.tokens, 0, "格式不符时绝不能编出数字");

        std::fs::remove_file(database).ok();
        std::fs::remove_dir_all(dir).ok();
    }
}
