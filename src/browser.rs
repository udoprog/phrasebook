//! The user's language in the browser: the remembered choice in
//! localStorage and the browser's preferences.
//!
//! An app's root opens its starting handle once,
//!
//! ```no_run
//! use phrasebook::{Lang, Phrasebook, browser};
//!
//! let book = Phrasebook::builder()
//!     .language(Lang::EN_US, "English")
//!     .language(Lang::new("sv-SE"), "Svenska")
//!     .build();
//!
//! let phrases = browser::initial(&book);
//! ```
//!
//! which also marks the page as being in that language. When the user picks
//! a language, the root provides `phrases.with_lang(lang)`, calls
//! [`set_document_lang`] with it, and calls
//! [`store`](crate::Phrases::store) on the new handle, so the choice survives
//! a reload. A change the user did not choose, such as a projector page
//! cycling through languages, skips `store`.
//!
//! Outside wasm (an app's host-side tests, say) these functions find
//! nothing, store nothing and mark nothing.

use crate::{Lang, Phrasebook, Phrases};

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

/// The handle to start with, open at the language remembered under
/// [`DEFAULT_KEY`], else the browser's preferences negotiated against the
/// phrasebook's languages ([`Phrasebook::negotiate`]), else the fallback.
/// Marks the page as being in that language, as [`set_document_lang`].
///
/// Pairs with [`Phrases::store`](crate::Phrases::store).
pub fn initial(book: &Phrasebook) -> Phrases {
    initial_with_key(book, DEFAULT_KEY)
}

/// [`initial`] with the language remembered under `key`, for an app
/// sharing its origin with another. Pairs with
/// [`Phrases::store_as`](crate::Phrases::store_as).
pub fn initial_with_key(book: &Phrasebook, key: &str) -> Phrases {
    let phrases = book.phrases(initial_lang(book, key));
    set_document_lang(phrases.lang());
    phrases
}

/// The language [`initial_with_key`] opens its handle at, for an app that
/// wants only the language.
pub fn initial_lang(book: &Phrasebook, key: &str) -> Lang {
    choose(book, load(key).as_deref(), &preferred())
}

/// Mark the page as being in `lang`: the `lang` attribute of its `<html>`
/// element, which screen readers, hyphenation and the browser's translation
/// offer read. [`initial`] does this for the starting language; call it on
/// every later language change, when providing `phrases.with_lang(lang)`.
pub fn set_document_lang(lang: Lang) {
    imp::set_document_lang(&lang.to_string());
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

    pub(super) fn set_document_lang(tag: &str) {
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            // Nothing to do about a page that refuses the attribute.
            let _ = root.set_attribute("lang", tag);
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

    pub(super) fn set_document_lang(_tag: &str) {}

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
        assert_eq!(initial_lang(&book, DEFAULT_KEY), Lang::EN_US);
        assert_eq!(initial(&book).lang(), Lang::EN_US);
        assert_eq!(initial_with_key(&book, "other").lang(), Lang::EN_US);
        book.phrases(Lang::new("sv-SE")).store();
    }
}
