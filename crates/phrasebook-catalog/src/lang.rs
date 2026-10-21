use core::fmt;
use core::str::FromStr;

/// A language identifier: a language subtag and an optional region subtag,
/// such as `en-US` or `sv`.
///
/// It is small and `Copy`, compares and hashes like the tag it spells, and
/// displays canonically, with the language in lower case and the region in
/// upper case. Only the shape of the tag is checked: two or three letters of
/// language, then optionally two letters of region.
///
/// # Examples
///
/// ```
/// use phrasebook_catalog::Lang;
///
/// const SV_SE: Lang = Lang::new("sv-SE");
///
/// assert_eq!(SV_SE.language(), "sv");
/// assert_eq!(SV_SE.region(), Some("SE"));
/// assert_eq!(SV_SE.primary(), Lang::new("sv"));
/// assert_eq!(SV_SE.to_string(), "sv-SE");
///
/// assert_eq!(Lang::parse("SV_se"), Some(SV_SE));
/// assert_eq!(Lang::parse("zh-Hant-TW"), Some(Lang::new("zh-TW")));
/// assert_eq!(Lang::parse("english"), None);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lang {
    /// Lower-case ASCII letters, padded with zeros.
    language: [u8; 3],
    /// Upper-case ASCII letters, or all zeros for no region.
    region: [u8; 2],
}

impl Lang {
    /// English as spoken in the United States, `en-US`.
    pub const EN_US: Lang = Lang::new("en-US");

    /// Construct a language from a canonical tag, `ll`, `lll`, `ll-RR` or
    /// `lll-RR`, for constants.
    ///
    /// # Panics
    ///
    /// Panics if the tag does not have that shape, which in a constant is a
    /// compile error. Use [`Lang::parse`] for tags from elsewhere.
    ///
    /// ```
    /// use phrasebook_catalog::Lang;
    ///
    /// const SV_SE: Lang = Lang::new("sv-SE");
    /// const FIL: Lang = Lang::new("fil");
    /// ```
    ///
    /// ```compile_fail
    /// use phrasebook_catalog::Lang;
    ///
    /// const BAD: Lang = Lang::new("sv_se");
    /// ```
    pub const fn new(tag: &str) -> Lang {
        let b = tag.as_bytes();
        let mut language = [0u8; 3];
        let mut i = 0;

        while i < b.len() && b[i] != b'-' {
            if i == 3 || !b[i].is_ascii_lowercase() {
                panic!("a language tag is `ll`, `lll`, `ll-RR` or `lll-RR`");
            }

            language[i] = b[i];
            i += 1;
        }

        if i < 2 {
            panic!("a language tag is `ll`, `lll`, `ll-RR` or `lll-RR`");
        }

        let mut region = [0u8; 2];

        if i < b.len() {
            if b.len() - i != 3 || !b[i + 1].is_ascii_uppercase() || !b[i + 2].is_ascii_uppercase()
            {
                panic!("a language tag is `ll`, `lll`, `ll-RR` or `lll-RR`");
            }

            region[0] = b[i + 1];
            region[1] = b[i + 2];
        }

        Lang { language, region }
    }

    /// Parse a tag leniently, as browsers and users write them: any case,
    /// `-` or `_` between subtags, and script, variant or extension subtags
    /// ignored. A numeric region such as `419` is ignored too.
    ///
    /// Returns `None` unless the tag starts with two or three letters.
    ///
    /// ```
    /// use phrasebook_catalog::Lang;
    ///
    /// assert_eq!(Lang::parse("en_gb"), Some(Lang::new("en-GB")));
    /// assert_eq!(Lang::parse("sv"), Some(Lang::new("sv")));
    /// assert_eq!(Lang::parse("es-419"), Some(Lang::new("es")));
    /// assert_eq!(Lang::parse("*"), None);
    /// ```
    pub fn parse(tag: &str) -> Option<Lang> {
        let mut subtags = tag.split(['-', '_']);

        let first = subtags.next()?.as_bytes();

        if !(2..=3).contains(&first.len()) || !first.iter().all(u8::is_ascii_alphabetic) {
            return None;
        }

        let mut language = [0u8; 3];

        for (to, from) in language.iter_mut().zip(first) {
            *to = from.to_ascii_lowercase();
        }

        let mut region = [0u8; 2];

        for subtag in subtags {
            let subtag = subtag.as_bytes();

            match subtag.len() {
                // A script subtag such as `Hant`: skip it and keep looking.
                4 if subtag.iter().all(u8::is_ascii_alphabetic) => continue,
                2 if subtag.iter().all(u8::is_ascii_alphabetic) => {
                    region[0] = subtag[0].to_ascii_uppercase();
                    region[1] = subtag[1].to_ascii_uppercase();
                    break;
                }
                _ => break,
            }
        }

        Some(Lang { language, region })
    }

    /// Build from a language code of two or three ASCII letters and an
    /// optional region of two, in any case. Returns `None` for any other
    /// shape.
    #[cfg(feature = "koine03")]
    pub(crate) fn from_parts(language: &str, region: Option<&str>) -> Option<Lang> {
        let language_bytes = language.as_bytes();

        if !(2..=3).contains(&language_bytes.len())
            || !language_bytes.iter().all(u8::is_ascii_alphabetic)
        {
            return None;
        }

        let mut lang = Lang {
            language: [0; 3],
            region: [0; 2],
        };

        for (to, from) in lang.language.iter_mut().zip(language_bytes) {
            *to = from.to_ascii_lowercase();
        }

        if let Some(region) = region {
            let &[a, b] = region.as_bytes() else {
                return None;
            };

            if !a.is_ascii_alphabetic() || !b.is_ascii_alphabetic() {
                return None;
            }

            lang.region = [a.to_ascii_uppercase(), b.to_ascii_uppercase()];
        }

        Some(lang)
    }

    /// The language subtag, such as `"sv"`.
    pub fn language(&self) -> &str {
        let len = self.language.iter().position(|&b| b == 0).unwrap_or(3);
        ascii(&self.language[..len])
    }

    /// The region subtag, such as `Some("SE")`.
    pub fn region(&self) -> Option<&str> {
        if self.region[0] == 0 {
            None
        } else {
            Some(ascii(&self.region))
        }
    }

    /// The language without its region, such as `sv` for `sv-SE`.
    pub fn primary(&self) -> Lang {
        Lang {
            language: self.language,
            region: [0; 2],
        }
    }
}

fn ascii(bytes: &[u8]) -> &str {
    // Only ASCII letters are ever stored.
    core::str::from_utf8(bytes).unwrap_or_default()
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.language())?;

        if let Some(region) = self.region() {
            write!(f, "-{region}")?;
        }

        Ok(())
    }
}

impl fmt::Debug for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Parses as [`Lang::parse`] does.
///
/// ```
/// use phrasebook_catalog::Lang;
///
/// let lang: Lang = "sv-se".parse()?;
/// assert_eq!(lang, Lang::new("sv-SE"));
/// assert!("?".parse::<Lang>().is_err());
/// # Ok::<_, phrasebook_catalog::ParseLangError>(())
/// ```
impl FromStr for Lang {
    type Err = ParseLangError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Lang::parse(s).ok_or(ParseLangError)
    }
}

/// The error when a string is not a language tag.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseLangError;

impl fmt::Display for ParseLangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a language tag")
    }
}

impl core::error::Error for ParseLangError {}

#[cfg(test)]
mod tests {
    use std::format;
    use std::string::ToString;

    use super::Lang;

    #[test]
    fn new_and_display() {
        assert_eq!(Lang::EN_US.to_string(), "en-US");
        assert_eq!(Lang::new("fil").to_string(), "fil");
        assert_eq!(Lang::new("fil-PH").language(), "fil");
        assert_eq!(Lang::new("sv").region(), None);
        assert_eq!(format!("{:?}", Lang::new("sv-SE")), "sv-SE");
    }

    #[test]
    #[should_panic]
    fn new_rejects_lowercase_region() {
        let _ = Lang::new("sv-se");
    }

    #[test]
    #[should_panic]
    fn new_rejects_long_language() {
        let _ = Lang::new("abcd");
    }

    #[test]
    fn parse() {
        assert_eq!(Lang::parse("en"), Some(Lang::new("en")));
        assert_eq!(Lang::parse("EN-us"), Some(Lang::EN_US));
        assert_eq!(Lang::parse("en_US"), Some(Lang::EN_US));
        assert_eq!(Lang::parse("zh-Hant-TW"), Some(Lang::new("zh-TW")));
        assert_eq!(Lang::parse("zh-Hant"), Some(Lang::new("zh")));
        assert_eq!(Lang::parse("de-CH-1996"), Some(Lang::new("de-CH")));
        assert_eq!(Lang::parse("en-u-ca-gregory"), Some(Lang::new("en")));
        assert_eq!(Lang::parse("es-419"), Some(Lang::new("es")));
        assert_eq!(Lang::parse(""), None);
        assert_eq!(Lang::parse("x-private"), None);
        assert_eq!(Lang::parse("e1"), None);
        assert_eq!(Lang::parse("engl"), None);
    }

    #[test]
    fn ordering_and_primary() {
        assert!(Lang::new("en") < Lang::new("en-US"));
        assert_eq!(Lang::new("sv-SE").primary(), Lang::new("sv"));
        assert_eq!(Lang::new("sv").primary(), Lang::new("sv"));
    }
}
