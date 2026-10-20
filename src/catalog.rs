use crate::Lang;

/// One language's Fluent source, usually embedded with `include_str!`.
///
/// It is parsed only when a bundle in its language is first needed.
#[derive(Debug, Clone, Copy)]
pub struct Source {
    lang: Lang,
    ftl: &'static str,
}

impl Source {
    /// The Fluent source `ftl` for `lang`.
    pub const fn new(lang: Lang, ftl: &'static str) -> Source {
        Source { lang, ftl }
    }

    /// The language of this source.
    pub const fn lang(&self) -> Lang {
        self.lang
    }

    /// The Fluent source text.
    pub const fn ftl(&self) -> &'static str {
        self.ftl
    }
}

/// Everything one crate ships: its name, the prefix every id it defines must
/// start with (if any), and one [`Source`] per language.
///
/// The first source is the catalog's default language, which
/// [`Phrases::from_catalog`] and [`Phrases::with_backstop`] fall back to.
///
/// [`Phrases::from_catalog`]: crate::Phrases::from_catalog
/// [`Phrases::with_backstop`]: crate::Phrases::with_backstop
///
/// # Examples
///
/// An app's catalog:
///
/// ```
/// use phrasebook::{Catalog, Lang, Source};
///
/// const SV_SE: Lang = Lang::new("sv-SE");
///
/// static L10N: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, "hello = Hello"),
///     Source::new(SV_SE, "hello = Hej"),
/// ]);
///
/// assert_eq!(L10N.languages().collect::<Vec<_>>(), [Lang::EN_US, SV_SE]);
/// ```
///
/// A library's, whose ids all start with its prefix:
///
/// ```
/// use phrasebook::{Catalog, Lang, Source};
///
/// pub static L10N: Catalog = Catalog::new("watchword", &[
///     Source::new(Lang::EN_US, "watchword-sign-in = Sign in"),
/// ])
/// .with_prefix("watchword-");
///
/// assert_eq!(L10N.prefix(), Some("watchword-"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Catalog {
    name: &'static str,
    prefix: Option<&'static str>,
    sources: &'static [Source],
}

impl Catalog {
    /// A catalog named `name` (used in logs and check reports) with one
    /// source per language, the default language first.
    pub const fn new(name: &'static str, sources: &'static [Source]) -> Catalog {
        Catalog {
            name,
            prefix: None,
            sources,
        }
    }

    /// Require every message and term id this catalog defines to start with
    /// `prefix`. A violation is logged when a bundle is built and fails
    /// [`check::catalog`](crate::check::catalog).
    pub const fn with_prefix(self, prefix: &'static str) -> Catalog {
        Catalog {
            prefix: Some(prefix),
            ..self
        }
    }

    /// The catalog's name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The prefix its ids must have, if any.
    pub const fn prefix(&self) -> Option<&'static str> {
        self.prefix
    }

    /// Its sources, the default language first.
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }

    /// The catalog's default language, that of its first source.
    pub fn default_lang(&self) -> Option<Lang> {
        self.sources.first().map(Source::lang)
    }

    /// The languages it has sources for, in order.
    pub fn languages(&self) -> impl Iterator<Item = Lang> {
        self.sources.iter().map(Source::lang)
    }

    /// The source for `lang`, the first if there are several.
    pub fn source(&self, lang: Lang) -> Option<&'static Source> {
        self.sources.iter().find(|s| s.lang == lang)
    }

    /// Whether `id` (a message id, or a term id without its leading `-`) is
    /// inside this catalog's prefix.
    #[cfg(feature = "std")]
    pub(crate) fn owns(&self, id: &str) -> bool {
        self.prefix.is_none_or(|prefix| id.starts_with(prefix))
    }
}
