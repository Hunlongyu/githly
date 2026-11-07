use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::{Arc, LazyLock, RwLock};

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

/// A label kept in a repository snapshot or UI state across language changes.
#[derive(Clone, Debug)]
pub enum LocalizedText {
    Verbatim(String),
    Translated {
        english: Cow<'static, str>,
        chinese: Cow<'static, str>,
    },
    Diagnostic {
        error: Arc<anyhow::Error>,
        render: fn(Locale, &anyhow::Error) -> String,
    },
}

impl LocalizedText {
    pub fn new<T: Into<Cow<'static, str>>>(render: impl Fn(Locale) -> T) -> Self {
        Self::Translated {
            english: render(Locale::English).into(),
            chinese: render(Locale::Chinese).into(),
        }
    }

    pub fn diagnostic(error: anyhow::Error) -> Self {
        Self::with_error(error, |_, error| format!("{error:#}"))
    }

    pub fn with_error(error: anyhow::Error, render: fn(Locale, &anyhow::Error) -> String) -> Self {
        Self::Diagnostic {
            error: Arc::new(error),
            render,
        }
    }

    pub fn get(&self, locale: Locale) -> Cow<'_, str> {
        match self {
            Self::Verbatim(text) => text.as_str().into(),
            Self::Translated { english, chinese } => match locale {
                Locale::English => english.as_ref().into(),
                Locale::Chinese => chinese.as_ref().into(),
            },
            Self::Diagnostic { error, render } => render(locale, error).into(),
        }
    }
}

impl std::fmt::Display for LocalizedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.get(messages()))
    }
}

impl std::error::Error for LocalizedText {}

impl PartialEq for LocalizedText {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Verbatim(a), Self::Verbatim(b)) => a == b,
            (
                Self::Translated {
                    english: a,
                    chinese: b,
                },
                Self::Translated {
                    english: c,
                    chinese: d,
                },
            ) => a == c && b == d,
            (
                Self::Diagnostic {
                    error: a,
                    render: f,
                },
                Self::Diagnostic {
                    error: b,
                    render: g,
                },
            ) => Arc::ptr_eq(a, b) && std::ptr::fn_addr_eq(*f, *g),
            _ => false,
        }
    }
}
impl Eq for LocalizedText {}

impl From<String> for LocalizedText {
    fn from(text: String) -> Self {
        Self::Verbatim(text)
    }
}

impl From<&str> for LocalizedText {
    fn from(text: &str) -> Self {
        Self::Verbatim(text.to_owned())
    }
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
static ACTIVE: LazyLock<RwLock<Locale>> = LazyLock::new(|| RwLock::new(*SYSTEM));

impl Language {
    pub fn resolve(self) -> Locale {
        match self {
            Self::System => *SYSTEM,
            Self::Chinese => Locale::Chinese,
            Self::English => Locale::English,
        }
    }
}

pub fn set_language(preference: Language) -> Locale {
    let locale = preference.resolve();
    *ACTIVE.write().unwrap() = locale;
    gpui_kit::component::set_locale(locale.tag());
    locale
}

pub fn messages() -> Locale {
    *ACTIVE.read().unwrap()
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
