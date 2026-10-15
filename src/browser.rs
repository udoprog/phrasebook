//! The user's language in the browser: the remembered choice in
//! localStorage and the browser's preferences.
//!
//! An app's root picks its starting language once,
//!
//! ```no_run
//! use phrasebook::{Lang, Phrasebook, browser};
//!
//! let book = Phrasebook::builder()
//!     .language(Lang::EN_US, "English")
//!     .language(Lang::new("sv-SE"), "Svenska")
//!     .build();
//!
//! let phrases = book.phrases(browser::initial(&book, browser::DEFAULT_KEY));
//! ```
//!
//! and when the user picks a language, provides `phrases.with_lang(lang)`
//! and calls [`store`](crate::Phrases::store) on it, so the choice survives a
//! reload. A change the user did not choose, such as a projector page
//! cycling through languages, calls `with_lang` alone.
//!
//! Outside wasm (an app's host-side tests, say) these functions find
//! nothing and store nothing.

use crate::{Lang, Phrasebook};

/// The localStorage key the language is remembered under by default. The
/// value is the canonical tag, such as `en-US`.
pub const DEFAULT_KEY: &str = "lang";

/// The language remembered under `key`, if it parses and the phrasebook
/// supports it.
pub fn remembered(book: &Phrasebook, key: &str) -> Option<Lang> {
    supported(book, load(key).as_deref())
}

/// The browser's preferred languages (`navigator.languages`), most
/// preferred first. Falls back to `navigator.language` if the list is
/// empty.
pub fn preferred() -> Vec<String> {
    imp::preferred()
}

/// The language to start in: the language remembered under `key`, else the
/// browser's preferences negotiated against the phrasebook's languages
/// ([`Phrasebook::negotiate`]), else the fallback.
pub fn initial(book: &Phrasebook, key: &str) -> Lang {
    choose(book, load(key).as_deref(), &preferred())
}

fn supported(book: &Phrasebook, tag: Option<&str>) -> Option<Lang> {
    let lang = Lang::parse(tag?)?;
    book.supports(lang).then_some(lang)
}

fn choose(book: &Phrasebook, remembered: Option<&str>, preferred: &[String]) -> Lang {
    supported(book, remembered)
        .unwrap_or_else(|| book.negotiate(preferred.iter().map(String::as_str)))
}

fn load(key: &str) -> Option<String> {
    imp::load(key)
}

pub(crate) fn store(lang: Lang, key: &str) {
    imp::store(&lang.to_string(), key);
}

#[cfg(target_arch = "wasm32")]
mod imp {
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub(super) fn load(key: &str) -> Option<String> {
        storage()?.get_item(key).ok().flatten()
    }

    pub(super) fn store(value: &str, key: &str) {
        if let Some(storage) = storage() {
            // A full or disabled storage only loses the choice on reload.
            let _ = storage.set_item(key, value);
        }
    }

    pub(super) fn preferred() -> Vec<String> {
        let Some(window) = web_sys::window() else {
            return Vec::new();
        };

        let navigator = window.navigator();

        let mut out = navigator
            .languages()
            .iter()
            .filter_map(|value| value.as_string())
            .collect::<Vec<_>>();

        if out.is_empty()
            && let Some(language) = navigator.language()
        {
            out.push(language);
        }

        out
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub(super) fn load(_key: &str) -> Option<String> {
        None
    }

    pub(super) fn store(_value: &str, _key: &str) {}

    pub(super) fn preferred() -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Phrasebook {
        Phrasebook::builder()
            .language(Lang::EN_US, "English")
            .language(Lang::new("sv-SE"), "Svenska")
            .build()
    }

    fn strings(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn remembered_choice_wins() {
        let book = book();
        let lang = choose(&book, Some("sv-SE"), &strings(&["en-US"]));
        assert_eq!(lang, Lang::new("sv-SE"));
    }

    #[test]
    fn unsupported_or_garbled_choice_is_ignored() {
        let book = book();
        assert_eq!(
            choose(&book, Some("de-DE"), &strings(&["sv"])),
            Lang::new("sv-SE")
        );
        assert_eq!(
            choose(&book, Some("???"), &strings(&["sv"])),
            Lang::new("sv-SE")
        );
    }

    #[test]
    fn preferences_are_negotiated() {
        let book = book();
        // A user preferring German, then Swedish, gets Swedish.
        assert_eq!(
            choose(&book, None, &strings(&["de", "sv"])),
            Lang::new("sv-SE")
        );
        assert_eq!(choose(&book, None, &strings(&["en-GB"])), Lang::EN_US);
        assert_eq!(
            choose(&book, None, &strings(&["sv_FI", "en"])),
            Lang::new("sv-SE")
        );
        assert_eq!(choose(&book, None, &[]), Lang::EN_US);
    }

    #[test]
    fn host_finds_and_stores_nothing() {
        let book = book();
        store(Lang::new("sv-SE"), DEFAULT_KEY);
        assert_eq!(remembered(&book, DEFAULT_KEY), None);
        assert!(preferred().is_empty());
        assert_eq!(initial(&book, DEFAULT_KEY), Lang::EN_US);
        book.phrases(Lang::new("sv-SE")).store();
    }
}
