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
//! With this feature on, [`Builder::build`] also warns about a declared
//! language koine does not know (see [`is_known`]), and
//! [`Builder::language_named_by_koine`] declares a language under koine's
//! English reference name.
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
//! [`Builder::build`]: crate::Builder::build
//! [`Builder::language_named_by_koine`]: crate::Builder::language_named_by_koine

use core::fmt;

use ::koine03::{Country, Language, Locale};

use crate::Lang;

/// The koine language of a [`Lang`], or `None` if koine does not know it.
///
/// ```
/// # extern crate koine03 as koine;
/// use koine::Language;
/// use phrasebook::{Lang, koine03};
///
/// assert_eq!(koine03::language(Lang::EN_US), Some(Language::ENG));
/// assert_eq!(koine03::language(Lang::new("fil")), Some(Language::new(b"fil")));
/// assert_eq!(koine03::language(Lang::new("xx")), None);
/// ```
pub fn language(lang: Lang) -> Option<Language> {
    Language::from_code(lang.language())
}

/// The koine country of a [`Lang`]'s region, or `None` if it has no region
/// or koine does not know it.
///
/// ```
/// # extern crate koine03 as koine;
/// use koine::Country;
/// use phrasebook::{Lang, koine03};
///
/// assert_eq!(koine03::country(Lang::EN_US), Some(Country::US));
/// assert_eq!(koine03::country(Lang::new("en")), None);
/// assert_eq!(koine03::country(Lang::new("en-ZZ")), None);
/// ```
pub fn country(lang: Lang) -> Option<Country> {
    Country::from_code(lang.region()?)
}

/// The koine locale of a [`Lang`], the same as [`Locale::try_from`]: `None`
/// if koine does not know the language, or the region when there is one.
///
/// Whether the pair is one koine lists as a valid locale is
/// [`is_known`]'s question.
///
/// ```
/// # extern crate koine03 as koine;
/// use koine::Locale;
/// use phrasebook::{Lang, koine03};
///
/// assert_eq!(koine03::locale(Lang::EN_US), Some(Locale::EN_US));
/// assert_eq!(koine03::locale(Lang::new("en-ZZ")), None);
/// ```
pub fn locale(lang: Lang) -> Option<Locale> {
    let country = match lang.region() {
        Some(_) => country(lang)?,
        None => Country::DEFAULT,
    };

    Some(Locale::new(language(lang)?, country))
}

/// The [`Lang`] of a koine locale, the same as [`Lang::try_from`]: `None`
/// for [`Locale::DEFAULT`].
///
/// ```
/// # extern crate koine03 as koine;
/// use koine::{Country, Language, Locale};
/// use phrasebook::{Lang, koine03};
///
/// assert_eq!(koine03::lang(Locale::EN_US), Some(Lang::EN_US));
/// assert_eq!(koine03::lang(Locale::new(Language::new(b"fil"), Country::DEFAULT)), Some(Lang::new("fil")));
/// assert_eq!(koine03::lang(Locale::DEFAULT), None);
/// ```
pub fn lang(locale: Locale) -> Option<Lang> {
    let language = locale.language().to_tag()?;

    let country = locale.country();
    let region = (!country.is_default()).then(|| country.as_str());

    Lang::from_parts(language, region)
}

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

/// Fails if koine does not know the language, or the region when there is
/// one.
impl TryFrom<Lang> for Locale {
    type Error = Error;

    fn try_from(lang: Lang) -> Result<Self, Self::Error> {
        let Some(language) = language(lang) else {
            return Err(Error(Kind::Language));
        };

        let country = match lang.region() {
            Some(_) => country(lang).ok_or(Error(Kind::Country))?,
            None => Country::DEFAULT,
        };

        Ok(Locale::new(language, country))
    }
}

/// Fails for [`Locale::DEFAULT`].
impl TryFrom<Locale> for Lang {
    type Error = Error;

    fn try_from(locale: Locale) -> Result<Self, Self::Error> {
        lang(locale).ok_or(Error(Kind::Default))
    }
}

/// A [`Lang`] without a region. Fails for [`Language::DEFAULT`].
///
/// ```
/// # extern crate koine03 as koine;
/// use koine::Language;
/// use phrasebook::Lang;
///
/// assert_eq!(Lang::try_from(Language::ENG), Ok(Lang::new("en")));
/// assert!(Lang::try_from(Language::DEFAULT).is_err());
/// ```
impl TryFrom<Language> for Lang {
    type Error = Error;

    fn try_from(language: Language) -> Result<Self, Self::Error> {
        Lang::try_from(Locale::new(language, Country::DEFAULT))
    }
}

/// The error when a [`Lang`] and a koine identifier do not convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(Kind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Language,
    Country,
    Default,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Kind::Language => f.write_str("koine does not know the language"),
            Kind::Country => f.write_str("koine does not know the region"),
            Kind::Default => f.write_str("koine's default sentinel names no language"),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
mod tests {
    use ::koine03::{Country, Language, Locale};

    use super::*;

    const SV_SE: Lang = Lang::new("sv-SE");

    #[test]
    fn lang_to_locale() {
        assert_eq!(Locale::try_from(Lang::EN_US), Ok(Locale::EN_US));
        assert_eq!(
            Locale::try_from(Lang::new("sv")),
            Ok(Locale::new(Language::new(b"swe"), Country::DEFAULT))
        );
        // A 3-letter code with a 2-letter form is the same language.
        assert_eq!(
            Locale::try_from(Lang::new("swe-SE")),
            Locale::try_from(SV_SE)
        );
        assert_eq!(
            Locale::try_from(Lang::new("xx-SE")),
            Err(Error(Kind::Language))
        );
        assert_eq!(
            Locale::try_from(Lang::new("sv-ZZ")),
            Err(Error(Kind::Country))
        );
        assert_eq!(locale(SV_SE), Locale::try_from(SV_SE).ok());
        assert_eq!(locale(Lang::new("sv-ZZ")), None);
    }

    #[test]
    fn locale_to_lang() {
        assert_eq!(Lang::try_from(Locale::EN_US), Ok(Lang::EN_US));
        assert_eq!(Lang::try_from(Locale::UNKNOWN), Ok(Lang::new("und")));
        assert_eq!(
            Lang::try_from(Locale::new(Language::new(b"swe"), Country::new(b"FI"))),
            Ok(Lang::new("sv-FI"))
        );
        // No ISO 639-1 code: the ISO 639-3 code.
        assert_eq!(
            Lang::try_from(Locale::new(Language::new(b"fil"), Country::new(b"PH"))),
            Ok(Lang::new("fil-PH"))
        );
        assert_eq!(Lang::try_from(Locale::DEFAULT), Err(Error(Kind::Default)));
        // A country without a language is still no language.
        assert_eq!(
            Lang::try_from(Locale::new(Language::DEFAULT, Country::US)),
            Err(Error(Kind::Default))
        );
        assert_eq!(lang(Locale::EN_US), Some(Lang::EN_US));
    }

    #[test]
    fn language_to_lang() {
        assert_eq!(Lang::try_from(Language::new(b"swe")), Ok(Lang::new("sv")));
        assert_eq!(Lang::try_from(Language::UND), Ok(Lang::new("und")));
        assert_eq!(Lang::try_from(Language::DEFAULT), Err(Error(Kind::Default)));
    }

    #[test]
    fn round_trips() {
        for tag in ["en-US", "sv-SE", "sv", "fil-PH", "pt-BR", "zh-TW", "und"] {
            let lang = Lang::new(tag);
            let locale = Locale::try_from(lang).unwrap();
            assert_eq!(Lang::try_from(locale), Ok(lang), "{tag}");
            assert_eq!(locale.to_string(), tag, "{tag}");
        }

        for locale in ::koine03::locale::iter().step_by(13) {
            let locale = Locale::new(
                Language::new(locale.language.as_bytes()),
                Country::new(locale.country.as_bytes()),
            );
            let lang = Lang::try_from(locale).unwrap();
            assert_eq!(Locale::try_from(lang), Ok(locale), "{locale}");
            assert_eq!(lang.to_string(), locale.to_string(), "{locale}");
            assert!(is_known(lang), "{lang}");
        }
    }

    #[test]
    fn parts() {
        assert_eq!(language(SV_SE), Some(Language::new(b"swe")));
        assert_eq!(language(Lang::new("xx")), None);
        assert_eq!(country(SV_SE), Some(Country::new(b"SE")));
        assert_eq!(country(Lang::new("sv")), None);
        assert_eq!(country(Lang::new("sv-ZZ")), None);
        assert_eq!(name(SV_SE), Some("Swedish"));
        assert_eq!(name(Lang::new("en-GB")), Some("English"));
        assert_eq!(name(Lang::new("xx")), None);
    }

    #[test]
    fn known() {
        assert!(is_known(Lang::EN_US));
        assert!(is_known(Lang::new("en")));
        assert!(is_known(Lang::new("fil")));
        assert!(!is_known(Lang::new("xx")));
        assert!(!is_known(Lang::new("en-ZZ")));
        assert!(!is_known(Lang::new("sv-JP")));
    }

    #[test]
    fn error_messages() {
        assert_eq!(
            Error(Kind::Language).to_string(),
            "koine does not know the language"
        );
        assert_eq!(
            Error(Kind::Country).to_string(),
            "koine does not know the region"
        );
        assert_eq!(
            Error(Kind::Default).to_string(),
            "koine's default sentinel names no language"
        );
    }
}
