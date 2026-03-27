//! 国际化支持
//!
//! 通过环境变量 `MARGI_LANG` / `LANG` / `LANGUAGE` / `LC_ALL` 自动检测语言。
//! 默认英文；zh_* 系统自动切换中文。
//!
//! 只需在 main.rs 中 `#[macro_use] mod i18n;`（必须是第一个 mod 声明），
//! 整个 crate 所有文件即可直接调用 `t!`，无需任何 import。

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale { Zh, En }

static LOCALE: OnceLock<Locale> = OnceLock::new();

pub fn init() { let _ = LOCALE.set(detect()); }

pub fn locale() -> Locale { *LOCALE.get_or_init(detect) }

fn detect() -> Locale {
    let val = std::env::var("MARGI_LANG")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LANGUAGE"))
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default()
        .to_lowercase();
    if val.starts_with("zh") { Locale::Zh } else { Locale::En }
}

/// 根据当前语言环境返回对应字符串。
/// 不使用 `#[macro_export]`，通过 `#[macro_use] mod i18n` 在 crate 内传播。
macro_rules! t {
    ($zh:expr, $en:expr) => {
        match $crate::i18n::locale() {
            $crate::i18n::Locale::Zh => $zh,
            $crate::i18n::Locale::En => $en,
        }
    };
}
