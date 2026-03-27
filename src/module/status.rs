use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleStatus {
    Unknown,
    Analyzing { since: DateTime<Utc> },
    Partial   { since: DateTime<Utc> },
    Understood{ since: DateTime<Utc> },
    Outdated  { since: DateTime<Utc>, reason: String },
}

impl ModuleStatus {
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        if raw == "unknown" || raw.is_empty() {
            return ModuleStatus::Unknown;
        }
        let parts: Vec<&str> = raw.splitn(3, ':').collect();
        let ts = parts.get(1)
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(|t| DateTime::from_timestamp(t, 0))
            .unwrap_or_else(Utc::now);

        match parts[0] {
            "analyzing"  => ModuleStatus::Analyzing  { since: ts },
            "partial"    => ModuleStatus::Partial    { since: ts },
            "understood" => ModuleStatus::Understood { since: ts },
            "outdated"   => ModuleStatus::Outdated   { since: ts, reason: parts.get(2).unwrap_or(&"").to_string() },
            _            => ModuleStatus::Unknown,
        }
    }

    pub fn to_file_content(&self) -> String {
        match self {
            ModuleStatus::Unknown              => "unknown".to_string(),
            ModuleStatus::Analyzing { since }  => format!("analyzing:{}", since.timestamp()),
            ModuleStatus::Partial   { since }  => format!("partial:{}", since.timestamp()),
            ModuleStatus::Understood{ since }  => format!("understood:{}", since.timestamp()),
            ModuleStatus::Outdated  { since, reason } => format!("outdated:{}:{}", since.timestamp(), reason),
        }
    }

    /// 用于文件写入和内部逻辑判断的固定 ASCII 标签（不参与 i18n）
    pub fn label(&self) -> &str {
        match self {
            ModuleStatus::Unknown          => "unknown",
            ModuleStatus::Analyzing { .. } => "analyzing",
            ModuleStatus::Partial { .. }   => "partial",
            ModuleStatus::Understood { .. }=> "understood",
            ModuleStatus::Outdated { .. }  => "outdated",
        }
    }

    /// 用于用户界面展示的本地化标签
    pub fn label_i18n(&self) -> String {
        let ts_fmt = |ts: &DateTime<Utc>| ts.format("%Y-%m-%d").to_string();
        match self {
            ModuleStatus::Unknown => {
                t!("未分析", "unknown").to_string()
            }
            ModuleStatus::Analyzing { since } => {
                t!(
                    format!("分析中 ({})", ts_fmt(since)),
                    format!("analyzing ({})", ts_fmt(since))
                )
            }
            ModuleStatus::Partial { since } => {
                t!(
                    format!("部分完成 ({})", ts_fmt(since)),
                    format!("partial ({})", ts_fmt(since))
                )
            }
            ModuleStatus::Understood { since } => {
                t!(
                    format!("已理解 ({})", ts_fmt(since)),
                    format!("understood ({})", ts_fmt(since))
                )
            }
            ModuleStatus::Outdated { since, reason } => {
                let base = t!(
                    format!("已过时 ({})", ts_fmt(since)),
                    format!("outdated ({})", ts_fmt(since))
                );
                if reason.is_empty() { base } else { format!("{} — {}", base, reason) }
            }
        }
    }

    #[allow(dead_code)]
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            ModuleStatus::Unknown          => None,
            ModuleStatus::Analyzing { since }  => Some(*since),
            ModuleStatus::Partial   { since }  => Some(*since),
            ModuleStatus::Understood{ since }  => Some(*since),
            ModuleStatus::Outdated  { since, .. } => Some(*since),
        }
    }
}

impl fmt::Display for ModuleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label_i18n())
    }
}
