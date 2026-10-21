//! Conversions between [`Lang`] and koine 0.3's compact identifiers.
//!
//! [koine] has `Copy` value types over the ISO tables: a [`Language`]
//! (ISO 639-3), a [`Country`] (ISO 3166-1 alpha-2) and a [`Locale`] (a
//! language and an optional country). A [`Lang`] converts to and from a
//! [`Locale`] with [`TryFrom`], or with [`locale`] and [`lang`]; [`language`]
//! and [`country`] take its parts. A conversion fails where koine's tables
//! do not know the language or the region, and for koine's `DEFAULT`
//! sentinels, which name no language.
//!
//! A [`Lang`] made from koine spells its language with the ISO 639-1 code
//! where there is one and the ISO 639-3 code otherwise, as koine displays a
//! [`Locale`]: `swe` from koine becomes `sv`.
//!
//! The conversions, and [`Error`], are phrasebook-catalog's, which defines
//! [`Lang`] and so may implement koine's traits for it; this module
//! re-exports them beside [`is_known`] and [`name`], and this feature turns
//! on phrasebook-catalog's `koine03` feature.
//!
//! With this feature on, [`Builder::build`] also warns about a declared
//! language koine does not know (see [`is_known`]). An app with no name of
//! a language in itself to hand can declare it under koine's English
//! reference name, [`name`]:
//!
//! ```
//! use phrasebook::{Lang, Phrasebook, koine03};
//!
//! const SV_SE: Lang = Lang::new("sv-SE");
//!
//! let book = Phrasebook::builder()
//!     .language(SV_SE, koine03::name(SV_SE).unwrap_or("sv-SE"))
//!     .build();
//!
//! assert_eq!(book.languages()[0].name(), "Swedish");
//! ```
//!
//! ```
//! # extern crate koine03 as koine;
//! use koine::{Country, Language, Locale};
//! use phrasebook::Lang;
//! use phrasebook::koine03;
//!
//! const SV_SE: Lang = Lang::new("sv-SE");
//!
//! let locale = Locale::try_from(SV_SE)?;
//! assert_eq!(locale.language(), Language::new(b"swe"));
//! assert_eq!(locale.country(), Country::new(b"SE"));
//! assert_eq!(Lang::try_from(locale)?, SV_SE);
//!
//! assert_eq!(koine03::country(SV_SE), Some(Country::new(b"SE")));
//! assert_eq!(koine03::name(SV_SE), Some("Swedish"));
//! assert!(koine03::is_known(SV_SE));
//! assert!(!koine03::is_known(Lang::new("sv-US")));
//! assert!(Locale::try_from(Lang::new("xx")).is_err());
//! # Ok::<_, koine03::Error>(())
//! ```
//!
//! [koine]: https://docs.rs/koine/0.3
//! [`Country`]: ::koine03::Country
//! [`Language`]: ::koine03::Language
//! [`Locale`]: ::koine03::Locale
//! [`Builder::build`]: crate::Builder::build

use crate::Lang;

#[doc(inline)]
pub use phrasebook_catalog::koine03::{Error, country, lang, language, locale};

/// Whether koine knows a [`Lang`]: its language is in koine's table and,
/// when it has a region, the language and country are a combination koine
/// lists as a valid locale.
///
/// ```
/// use phrasebook::{Lang, koine03};
///
/// assert!(koine03::is_known(Lang::new("sv")));
/// assert!(koine03::is_known(Lang::new("sv-FI")));
/// assert!(!koine03::is_known(Lang::new("sv-JP")));
/// assert!(!koine03::is_known(Lang::new("xx")));
/// ```
pub fn is_known(lang: Lang) -> bool {
    match locale(lang) {
        Some(locale) if lang.region().is_some() => locale.is_listed(),
        Some(_) => true,
        None => false,
    }
}

/// koine's English reference name for a [`Lang`]'s language, or `None` if
/// koine does not know it. The region is not part of the name, so `en-US`
/// and `en-GB` are both `"English"`.
///
/// ```
/// use phrasebook::{Lang, koine03};
///
/// assert_eq!(koine03::name(Lang::new("sv-SE")), Some("Swedish"));
/// assert_eq!(koine03::name(Lang::new("xx")), None);
/// ```
pub fn name(lang: Lang) -> Option<&'static str> {
    language(lang)?.name()
}

#[cfg(test)]
mod tests {
    use ::koine03::{Country, Language, Locale};

    use super::*;

    #[test]
    fn known() {
        assert!(is_known(Lang::EN_US));
        assert!(is_known(Lang::new("en")));
        assert!(is_known(Lang::new("fil")));
        assert!(!is_known(Lang::new("xx")));
        assert!(!is_known(Lang::new("en-ZZ")));
        assert!(!is_known(Lang::new("sv-JP")));

        for locale in ::koine03::locale::iter().step_by(13) {
            let locale = Locale::new(
                Language::new(locale.language.as_bytes()),
                Country::new(locale.country.as_bytes()),
            );
            let lang = Lang::try_from(locale).unwrap();
            assert!(is_known(lang), "{lang}");
        }
    }

    #[test]
    fn names() {
        assert_eq!(name(Lang::new("sv-SE")), Some("Swedish"));
        assert_eq!(name(Lang::new("en-GB")), Some("English"));
        assert_eq!(name(Lang::new("xx")), None);
    }
}
