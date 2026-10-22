//! Localization with Fluent. All user-facing text lives in `locales/*.ftl`.
//! German is the reference language.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

pub const DE_FTL: &str = include_str!("../../../locales/de.ftl");
pub const EN_FTL: &str = include_str!("../../../locales/en.ftl");

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// Map a BCP 47 locale string (`de-DE`, `de_AT.UTF-8`, `en-US`) to a
    /// supported language. Everything that is not German falls back to English.
    pub fn from_locale(locale: &str) -> Self {
        let l = locale.trim().to_ascii_lowercase();
        if l == "de" || l.starts_with("de-") || l.starts_with("de_") || l.starts_with("de.") {
            Self::De
        } else {
            Self::En
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "de" => Some(Self::De),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
        }
    }

    fn ftl(self) -> &'static str {
        match self {
            Self::De => DE_FTL,
            Self::En => EN_FTL,
        }
    }
}

/// A loaded string table for one language.
pub struct I18n {
    lang: Lang,
    bundle: FluentBundle<FluentResource>,
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18n").field("lang", &self.lang).finish()
    }
}

/// A Fluent argument value.
#[derive(Debug, Clone)]
pub enum Arg {
    Str(String),
    /// Numbers are formatted by [`I18n::num`] before they reach Fluent, so
    /// the decimal separator follows the language.
    Num(f64),
}

impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Arg::Str(s.to_string())
    }
}
impl From<String> for Arg {
    fn from(s: String) -> Self {
        Arg::Str(s)
    }
}
impl From<f64> for Arg {
    fn from(v: f64) -> Self {
        Arg::Num(v)
    }
}
impl From<u32> for Arg {
    fn from(v: u32) -> Self {
        Arg::Num(f64::from(v))
    }
}

impl I18n {
    pub fn new(lang: Lang) -> Self {
        let langid: LanguageIdentifier = lang.code().parse().expect("valid language id");
        let mut bundle = FluentBundle::new_concurrent(vec![langid]);
        bundle.set_use_isolating(false);
        let res = FluentResource::try_new(lang.ftl().to_string())
            .unwrap_or_else(|(res, _errors)| res);
        // Duplicate ids are a test failure (see tests below), never a panic.
        let _ = bundle.add_resource(res);
        Self { lang, bundle }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Format a number with the decimal separator of the language.
    pub fn num(&self, v: f64) -> String {
        crate::types::fmt_number(v, self.lang == Lang::De)
    }

    /// Look up a message. Missing ids return the id itself, so a gap is
    /// visible but never crashes. Tests make sure no id is missing.
    pub fn t(&self, id: &str, args: &[(&str, Arg)]) -> String {
        let Some(msg) = self.bundle.get_message(id) else {
            return id.to_string();
        };
        let Some(pattern) = msg.value() else {
            return id.to_string();
        };
        let mut fargs = FluentArgs::new();
        for (k, v) in args {
            match v {
                Arg::Str(s) => fargs.set(*k, FluentValue::from(s.clone())),
                Arg::Num(n) => fargs.set(*k, FluentValue::from(self.num(*n))),
            }
        }
        let mut errors = vec![];
        self.bundle
            .format_pattern(pattern, Some(&fargs), &mut errors)
            .into_owned()
    }

    pub fn has(&self, id: &str) -> bool {
        self.bundle.has_message(id)
    }
}

/// Message ids defined in an FTL source (top-level messages only).
pub fn message_ids(ftl: &str) -> Vec<String> {
    ftl.lines()
        .filter(|l| {
            l.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
        })
        .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn both_languages_parse_without_errors() {
        for ftl in [DE_FTL, EN_FTL] {
            let r = FluentResource::try_new(ftl.to_string());
            assert!(r.is_ok(), "FTL parse errors: {:?}", r.err().map(|e| e.1));
        }
    }

    #[test]
    fn both_languages_have_the_same_ids() {
        let de: BTreeSet<_> = message_ids(DE_FTL).into_iter().collect();
        let en: BTreeSet<_> = message_ids(EN_FTL).into_iter().collect();
        let only_de: Vec<_> = de.difference(&en).collect();
        let only_en: Vec<_> = en.difference(&de).collect();
        assert!(only_de.is_empty(), "only in de.ftl: {only_de:?}");
        assert!(only_en.is_empty(), "only in en.ftl: {only_en:?}");
    }

    #[test]
    fn no_duplicate_ids() {
        for ftl in [DE_FTL, EN_FTL] {
            let ids = message_ids(ftl);
            let set: BTreeSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), set.len(), "duplicate message ids");
        }
    }

    #[test]
    fn locale_mapping() {
        assert_eq!(Lang::from_locale("de-DE"), Lang::De);
        assert_eq!(Lang::from_locale("de_AT.UTF-8"), Lang::De);
        assert_eq!(Lang::from_locale("en-US"), Lang::En);
        assert_eq!(Lang::from_locale("fr-FR"), Lang::En);
    }

    #[test]
    fn decimal_comma_in_german() {
        let de = I18n::new(Lang::De);
        let s = de.t("power-contract", &[("watts", Arg::Num(27.5))]);
        assert!(s.contains("27,5"), "{s}");
        let en = I18n::new(Lang::En);
        let s = en.t("power-contract", &[("watts", Arg::Num(27.5))]);
        assert!(s.contains("27.5"), "{s}");
    }
}
