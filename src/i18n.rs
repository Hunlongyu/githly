use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, OnceLock};

mod catalog;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    System,
    English,
    Chinese,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    English,
    Chinese,
}

impl Locale {
    fn from_preferences(tags: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        tags.into_iter()
            .find_map(|tag| {
                let language = tag.as_ref().split(['-', '_', '.', '@']).next()?;
                if language.eq_ignore_ascii_case("zh") {
                    Some(Self::Chinese)
                } else if language.eq_ignore_ascii_case("en") {
                    Some(Self::English)
                } else {
                    None
                }
            })
            .unwrap_or(Self::English)
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
        }
    }
}

static SYSTEM: LazyLock<Locale> =
    LazyLock::new(|| Locale::from_preferences(sys_locale::get_locales()));
static ACTIVE: OnceLock<Locale> = OnceLock::new();

impl Language {
    pub fn resolve(self) -> Locale {
        match self {
            Self::System => *SYSTEM,
            Self::Chinese => Locale::Chinese,
            Self::English => Locale::English,
        }
    }
}

pub fn init(preference: Language) {
    ACTIVE
        .set(preference.resolve())
        .expect("language already initialized");
}

pub fn messages() -> Locale {
    ACTIVE.get().copied().unwrap_or_else(|| *SYSTEM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_render_labels_and_preserve_repository_text() {
        let en = Language::English.resolve();
        let zh = Language::Chinese.resolve();
        assert_eq!(en.settings(), "Settings");
        assert_eq!(zh.settings(), "设置");
        assert_eq!(en.history_scope("开发/feature"), "History · 开发/feature");
        assert_eq!(zh.history_scope("开发/feature"), "历史 · 开发/feature");
    }

    #[test]
    fn resolves_chinese_locales_and_falls_back_to_english() {
        for tag in ["zh", "zh-CN", "ZH-Hant-TW", "zh_HK.UTF-8"] {
            assert_eq!(Locale::from_preferences([tag]), Locale::Chinese);
        }
        for tag in ["en-US", "fr-FR", "C", ""] {
            assert_eq!(Locale::from_preferences([tag]), Locale::English);
        }
        assert_eq!(
            Locale::from_preferences(["fr", "zh-HK", "en"]),
            Locale::Chinese
        );
        assert_eq!(Locale::from_preferences(["en-GB", "zh"]), Locale::English);
        assert_eq!(Language::Chinese.resolve().tag(), "zh-CN");
        assert_eq!(Language::English.resolve().tag(), "en");
        assert_eq!(Language::System.resolve(), *SYSTEM);
    }
}
