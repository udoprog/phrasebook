use std::cell::{OnceCell, RefCell};
use std::collections::HashSet;
use std::collections::hash_map::{Entry, HashMap};
use std::fmt;
use std::ptr;
use std::rc::Rc;

use fluent_bundle::{FluentArgs, FluentError, FluentResource, FluentValue};
use fluent_syntax::ast;
use unic_langid::LanguageIdentifier;

use crate::{Args, Catalog, Lang, Phrases, Source};

/// Whether `id` (a message id, or a term id without its leading `-`) is
/// inside `catalog`'s prefix.
pub(crate) fn owns(catalog: &Catalog, id: &str) -> bool {
    catalog.prefix().is_none_or(|prefix| id.starts_with(prefix))
}

pub(crate) type Bundle = fluent_bundle::FluentBundle<FluentResource>;

/// One language's resolved plain texts by id.
type Texts = HashMap<Box<str>, Option<Rc<str>>>;

/// The variables a message selects on.
type Selectors = Rc<[Box<str>]>;

/// A supported language and its name in itself, for a language picker.
///
/// Cheap to clone. Its fields are private so that a name can later be
/// owned, for a language declared at runtime.
///
/// ```
/// use phrasebook::{Lang, Language};
///
/// let language = Language::new(Lang::new("sv-SE"), "Svenska");
/// assert_eq!(language.lang(), Lang::new("sv-SE"));
/// assert_eq!(language.name(), "Svenska");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Language {
    lang: Lang,
    name: &'static str,
}

impl Language {
    /// A language and its name in itself.
    pub const fn new(lang: Lang, name: &'static str) -> Language {
        Language { lang, name }
    }

    /// The language.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Its name in itself, such as `"Svenska"`.
    pub fn name(&self) -> &str {
        self.name
    }
}

/// The dictionary: every declared language's messages from every catalog.
///
/// Build one with [`Phrasebook::builder`] and hand out [`Phrases`] handles
/// with [`Phrasebook::phrases`]. Cloning is cheap: the bundles live behind
/// one `Rc` and two clones compare equal.
///
/// Each declared language gets one Fluent bundle holding every catalog's
/// source for it. A bundle is parsed when it is first needed, so an app
/// shipping five languages parses only the one it starts in (and the
/// fallback, once a message is missing).
///
/// # Examples
///
/// ```
/// use phrasebook::{Catalog, Lang, Phrasebook, Source};
///
/// const SV_SE: Lang = Lang::new("sv-SE");
///
/// static APP: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, "hello = Hello\nbye = Bye"),
///     Source::new(SV_SE, "hello = Hej"),
/// ]);
///
/// let book = Phrasebook::builder()
///     .language(Lang::EN_US, "English")
///     .language(SV_SE, "Svenska")
///     .catalog(&APP)
///     .build();
///
/// let sv = book.phrases(SV_SE);
/// assert_eq!(&*sv.text("hello"), "Hej");
/// // Missing in Swedish: the fallback language, English.
/// assert_eq!(&*sv.text("bye"), "Bye");
/// // Missing everywhere: the id itself.
/// assert_eq!(&*sv.text("nope"), "nope");
///
/// assert_eq!(book.negotiate(["de-DE", "sv-FI", "en"]), SV_SE);
/// ```
#[derive(Clone)]
pub struct Phrasebook {
    pub(crate) inner: Rc<Inner>,
}

impl Phrasebook {
    /// Start declaring a phrasebook.
    pub fn builder() -> Builder {
        Builder {
            languages: Vec::new(),
            fallback: None,
            catalogs: Vec::new(),
            use_isolating: false,
            locale_numbers: true,
        }
    }

    /// A handle in `lang`, or in the fallback language if `lang` is not
    /// declared.
    pub fn phrases(&self, lang: Lang) -> Phrases {
        Phrases::new(self.clone(), self.resolve(lang))
    }

    /// The declared languages, in the order they were declared.
    pub fn languages(&self) -> &[Language] {
        &self.inner.languages
    }

    /// The fallback language: where a message missing in a handle's language
    /// is looked up next.
    pub fn fallback(&self) -> Lang {
        self.inner.fallback
    }

    /// Whether `catalog` was added with [`Builder::catalog`], for an app's
    /// test that a library's catalog is in its phrasebook. Catalogs are
    /// compared by identity, so declare them as `static`s.
    ///
    /// ```
    /// use phrasebook::{Catalog, Lang, Phrasebook, Source};
    ///
    /// static APP: Catalog = Catalog::new("app", &[Source::new(Lang::EN_US, "title = App\n")]);
    /// static LIB: Catalog = Catalog::new("lib", &[Source::new(Lang::EN_US, "lib-ok = OK\n")]);
    ///
    /// let book = Phrasebook::builder()
    ///     .language(Lang::EN_US, "English")
    ///     .catalog(&APP)
    ///     .build();
    ///
    /// assert!(book.has_catalog(&APP));
    /// assert!(!book.has_catalog(&LIB));
    /// ```
    pub fn has_catalog(&self, catalog: &Catalog) -> bool {
        self.inner
            .catalogs
            .iter()
            .any(|added| ptr::eq(*added, catalog))
    }

    /// Whether `lang` is declared, or is the fallback.
    pub fn supports(&self, lang: Lang) -> bool {
        self.inner.index(lang).is_some()
    }

    /// The best supported language for a list of preferred tags, most
    /// preferred first, such as `navigator.languages`.
    ///
    /// Each tag is parsed leniently ([`Lang::parse`]) and, in order, matched
    /// exactly, then against the first declared language with the same
    /// language subtag. Without any match, the fallback.
    ///
    /// ```
    /// use phrasebook::{Lang, Phrasebook};
    ///
    /// let book = Phrasebook::builder()
    ///     .language(Lang::EN_US, "English")
    ///     .language(Lang::new("sv-SE"), "Svenska")
    ///     .build();
    ///
    /// assert_eq!(book.negotiate(["sv"]), Lang::new("sv-SE"));
    /// assert_eq!(book.negotiate(["en-GB", "sv-SE"]), Lang::EN_US);
    /// assert_eq!(book.negotiate(["de", "sv-SE"]), Lang::new("sv-SE"));
    /// assert_eq!(book.negotiate(["de"]), Lang::EN_US);
    /// ```
    pub fn negotiate<'a>(&self, preferred: impl IntoIterator<Item = &'a str>) -> Lang {
        for tag in preferred {
            let Some(wanted) = Lang::parse(tag) else {
                continue;
            };

            if self.supports(wanted) {
                return wanted;
            }

            let primary = wanted.primary();

            if let Some(language) = self
                .inner
                .languages
                .iter()
                .find(|l| l.lang.primary() == primary)
            {
                return language.lang;
            }
        }

        self.inner.fallback
    }

    /// `lang` if supported, else the fallback.
    pub(crate) fn resolve(&self, lang: Lang) -> Lang {
        if self.supports(lang) {
            lang
        } else {
            self.inner.fallback
        }
    }

    pub(crate) fn ptr_eq(&self, other: &Phrasebook) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Two phrasebooks are equal when they are clones of the same one.
impl PartialEq for Phrasebook {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for Phrasebook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Phrasebook")
            .field("languages", &self.inner.languages)
            .field("fallback", &self.inner.fallback)
            .field(
                "catalogs",
                &self
                    .inner
                    .catalogs
                    .iter()
                    .map(|c| c.name())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Declares a [`Phrasebook`]'s languages and catalogs.
#[must_use = "a builder does nothing until built"]
pub struct Builder {
    languages: Vec<Language>,
    fallback: Option<Lang>,
    catalogs: Vec<&'static Catalog>,
    use_isolating: bool,
    locale_numbers: bool,
}

impl Builder {
    /// Declare a supported language with its name in itself, in picker
    /// order. The first declared language is the fallback unless
    /// [`Builder::fallback`] says otherwise. Declaring a language again has
    /// no effect.
    pub fn language(mut self, lang: Lang, name: &'static str) -> Self {
        if self.languages.iter().all(|l| l.lang != lang) {
            self.languages.push(Language { lang, name });
        }

        self
    }

    /// Declare each language of a list with its name in itself, in order, as
    /// [`Builder::language`] does: the first is the fallback unless
    /// [`Builder::fallback`] says otherwise. For an app whose languages come
    /// from a table of its own.
    ///
    /// ```
    /// use phrasebook::{Lang, Phrasebook};
    ///
    /// const LANGUAGES: &[(Lang, &str)] = &[
    ///     (Lang::new("sv-SE"), "Svenska"),
    ///     (Lang::EN_US, "English"),
    /// ];
    ///
    /// let book = Phrasebook::builder()
    ///     .languages(LANGUAGES.iter().copied())
    ///     .build();
    ///
    /// assert_eq!(book.languages().len(), 2);
    /// assert_eq!(book.fallback(), Lang::new("sv-SE"));
    /// ```
    pub fn languages(self, languages: impl IntoIterator<Item = (Lang, &'static str)>) -> Self {
        languages
            .into_iter()
            .fold(self, |builder, (lang, name)| builder.language(lang, name))
    }

    /// Set the fallback language, for a fallback other than the first
    /// declared language, which is the fallback without this. It should be
    /// declared; if it is not, it still gets a bundle but is not listed in
    /// [`Phrasebook::languages`].
    pub fn fallback(mut self, lang: Lang) -> Self {
        self.fallback = Some(lang);
        self
    }

    /// Add a catalog. Earlier catalogs win a duplicate id, so an app adds
    /// its own catalog first and can override or translate a library's
    /// messages in it.
    ///
    /// A catalog's source for an undeclared language is ignored, and a
    /// declared language a catalog has no source for falls back for that
    /// catalog's messages.
    pub fn catalog(mut self, catalog: &'static Catalog) -> Self {
        self.catalogs.push(catalog);
        self
    }

    /// Whether to put Unicode bidi isolation marks around placeables. Off
    /// by default, since in HTML text nodes they show up as stray
    /// characters; a right-to-left app may want them on.
    pub fn use_isolating(mut self, on: bool) -> Self {
        self.use_isolating = on;
        self
    }

    /// Whether numbers are formatted in the handle's language, with the
    /// `browser` feature in the browser: `12345.5` as `12,345.5` in
    /// `en-US` and `12 345,5` in `sv-SE`, through the browser's
    /// `Intl.NumberFormat`, honouring `NUMBER()`'s options in a message
    /// (such as `minimumFractionDigits`, `useGrouping` and
    /// `style: "percent"`). On by default.
    ///
    /// Elsewhere, such as in an app's host-side tests, numbers are formatted
    /// as fluent does: no grouping and a `.` decimal separator, the same in
    /// every language.
    pub fn locale_numbers(mut self, on: bool) -> Self {
        self.locale_numbers = on;
        self
    }

    /// Build the phrasebook. Nothing is parsed yet.
    ///
    /// With the `koine03` feature, a declared language koine does not know
    /// (see [`koine03::is_known`](crate::koine03::is_known)) is logged as a
    /// warning.
    pub fn build(self) -> Phrasebook {
        #[cfg(feature = "koine03")]
        for language in &self.languages {
            if !crate::koine03::is_known(language.lang) {
                tracing::warn!(lang = %language.lang, "phrasebook: koine does not know the declared language");
            }
        }

        let fallback = self
            .fallback
            .or_else(|| self.languages.first().map(|l| l.lang))
            .unwrap_or(Lang::EN_US);

        let mut langs = self.languages.iter().map(|l| l.lang).collect::<Vec<_>>();

        if !langs.contains(&fallback) {
            if !self.languages.is_empty() {
                tracing::warn!(%fallback, "phrasebook: the fallback language is not declared");
            }

            langs.push(fallback);
        }

        let bundles = langs.iter().map(|_| OnceCell::new()).collect();

        Phrasebook {
            inner: Rc::new(Inner {
                languages: self.languages.into(),
                fallback,
                langs: langs.into(),
                bundles,
                catalogs: self.catalogs.into(),
                use_isolating: self.use_isolating,
                locale_numbers: self.locale_numbers,
                texts: RefCell::new(HashMap::new()),
                logged: RefCell::new(HashMap::new()),
                errors_logged: RefCell::new(HashSet::new()),
            }),
        }
    }
}

pub(crate) struct Inner {
    languages: Box<[Language]>,
    fallback: Lang,
    /// Every language with a bundle: the declared ones, and the fallback.
    langs: Box<[Lang]>,
    /// One lazily built bundle per entry in `langs`.
    bundles: Box<[OnceCell<LangBundle>]>,
    catalogs: Box<[&'static Catalog]>,
    use_isolating: bool,
    pub(crate) locale_numbers: bool,
    /// Plain texts, resolved once per language and id; `None` when neither
    /// the language nor the fallback defines the id.
    texts: RefCell<HashMap<Lang, Texts>>,
    /// Misses already logged, per language and id.
    logged: RefCell<HashMap<Lang, HashSet<Box<str>>>>,
    /// Ids whose format errors are already logged.
    errors_logged: RefCell<HashSet<Box<str>>>,
}

impl Inner {
    fn index(&self, lang: Lang) -> Option<usize> {
        self.langs.iter().position(|&l| l == lang)
    }

    fn bundle(&self, lang: Lang) -> Option<&LangBundle> {
        let index = self.index(lang)?;
        Some(self.bundles[index].get_or_init(|| self.build_bundle(lang)))
    }

    fn build_bundle(&self, lang: Lang) -> LangBundle {
        let mut bundle = new_bundle(lang, self.use_isolating, self.locale_numbers);
        // Where each id was first defined, by catalog index; terms keep
        // their leading `-`, as they are a namespace of their own.
        let mut seen = HashMap::<String, usize>::new();

        for (index, catalog) in self.catalogs.iter().enumerate() {
            let Some(source) = catalog.source(lang) else {
                continue;
            };

            let resource = parse(source, |line, error| {
                tracing::error!(
                    catalog = catalog.name(),
                    %lang,
                    line,
                    "phrasebook: syntax error: {error}"
                );
            });

            for (key, name) in resource.entries().filter_map(entry_id) {
                if !owns(catalog, name) {
                    tracing::warn!(
                        catalog = catalog.name(),
                        %lang,
                        id = key,
                        prefix = catalog.prefix().unwrap_or_default(),
                        "phrasebook: id outside the catalog's prefix"
                    );
                }

                match seen.entry(key.to_owned()) {
                    Entry::Vacant(e) => {
                        e.insert(index);
                    }
                    Entry::Occupied(e) if *e.get() == index => {
                        tracing::warn!(
                            catalog = catalog.name(),
                            %lang,
                            id = key,
                            "phrasebook: id defined twice; the first wins"
                        );
                    }
                    Entry::Occupied(e) => {
                        let first = self.catalogs[*e.get()].name();

                        // An earlier catalog defining an id inside a later
                        // catalog's prefix is an app overriding or
                        // translating a library's message on purpose.
                        if catalog.prefix().is_some() && owns(catalog, name) {
                            tracing::debug!(
                                catalog = catalog.name(),
                                by = first,
                                %lang,
                                id = key,
                                "phrasebook: message overridden by an earlier catalog"
                            );
                        } else {
                            tracing::warn!(
                                catalog = catalog.name(),
                                by = first,
                                %lang,
                                id = key,
                                "phrasebook: id already defined by an earlier catalog, which wins"
                            );
                        }
                    }
                }
            }

            // Overriding entries are reported above; the bundle keeps the
            // first definition of each.
            let _ = bundle.add_resource(resource);
        }

        LangBundle {
            bundle,
            selectors: RefCell::new(HashMap::new()),
        }
    }

    /// Format `id` in `lang`, then in the fallback. `quiet` leaves a miss
    /// in `lang` unlogged, for a backstop's lookups. Format errors are
    /// logged once per id either way.
    pub(crate) fn format(
        &self,
        lang: Lang,
        id: &str,
        args: Option<&Args<'_>>,
        quiet: bool,
    ) -> Option<String> {
        if let Some(text) = self.format_in(lang, id, args) {
            return Some(text);
        }

        if lang == self.fallback {
            return None;
        }

        let text = self.format_in(self.fallback, id, args)?;

        if !quiet && self.first_miss(lang, id) {
            tracing::warn!(
                %lang,
                fallback = %self.fallback,
                id,
                "phrasebook: message missing, using the fallback language"
            );
        }

        Some(text)
    }

    /// [`Inner::format`] without arguments, cached per language and id.
    pub(crate) fn text(&self, lang: Lang, id: &str, quiet: bool) -> Option<Rc<str>> {
        if let Some(text) = self.texts.borrow().get(&lang).and_then(|t| t.get(id)) {
            return text.clone();
        }

        let text = self.format(lang, id, None, quiet).map(Rc::<str>::from);

        self.texts
            .borrow_mut()
            .entry(lang)
            .or_default()
            .insert(Box::from(id), text.clone());

        text
    }

    fn format_in(&self, lang: Lang, id: &str, args: Option<&Args<'_>>) -> Option<String> {
        let mut errors = Vec::new();
        let text = self.bundle(lang)?.format(id, args, &mut errors)?;

        if !errors.is_empty() && self.errors_logged.borrow_mut().insert(Box::from(id)) {
            for error in errors {
                tracing::warn!(%lang, id, "phrasebook: format error: {error}");
            }
        }

        Some(text)
    }

    /// Whether this is the first miss of `id` in `lang` to be logged.
    pub(crate) fn first_miss(&self, lang: Lang, id: &str) -> bool {
        let mut logged = self.logged.borrow_mut();
        let logged = logged.entry(lang).or_default();
        !logged.contains(id) && logged.insert(Box::from(id))
    }
}

/// One language's bundle and the selector variables of the messages
/// formatted in it so far.
struct LangBundle {
    bundle: Bundle,
    selectors: RefCell<HashMap<Box<str>, Selectors>>,
}

impl LangBundle {
    fn format(
        &self,
        id: &str,
        args: Option<&Args<'_>>,
        errors: &mut Vec<FluentError>,
    ) -> Option<String> {
        let message = self.bundle.get_message(id)?;
        let pattern = message.value()?;

        let owned;

        let args = match args {
            Some(args) if args.has_strings() => {
                let selectors = self.selectors(id, pattern);
                owned = numbered(&args.inner, &selectors);
                owned.as_ref().or(Some(&args.inner))
            }
            Some(args) => Some(&args.inner),
            None => None,
        };

        Some(
            self.bundle
                .format_pattern(pattern, args, errors)
                .into_owned(),
        )
    }

    /// The variables `id` selects a variant on, worked out once.
    fn selectors(&self, id: &str, pattern: &ast::Pattern<&str>) -> Rc<[Box<str>]> {
        if let Some(selectors) = self.selectors.borrow().get(id) {
            return selectors.clone();
        }

        let mut out = Vec::new();
        selectors(pattern, &mut out);
        let selectors: Rc<[Box<str>]> = out.into_iter().map(Box::from).collect();

        self.selectors
            .borrow_mut()
            .insert(Box::from(id), selectors.clone());

        selectors
    }
}

/// A new empty bundle for `lang` with the built-in functions.
pub(crate) fn new_bundle(lang: Lang, use_isolating: bool, locale_numbers: bool) -> Bundle {
    let langid = lang
        .to_string()
        .parse::<LanguageIdentifier>()
        .unwrap_or_default();

    let mut bundle = Bundle::new(vec![langid]);
    bundle.set_use_isolating(use_isolating);
    // Only fails if a function is already defined, and the bundle is new.
    let _ = bundle.add_builtins();

    #[cfg(all(feature = "browser", target_arch = "wasm32"))]
    if locale_numbers {
        bundle.set_formatter(Some(crate::intl::format));
    }

    #[cfg(not(all(feature = "browser", target_arch = "wasm32")))]
    let _ = locale_numbers;

    bundle
}

/// Parse `source`, keeping whatever parsed and reporting each syntax error
/// with its 1-based line.
pub(crate) fn parse(
    source: &Source,
    mut report: impl FnMut(usize, &dyn fmt::Display),
) -> FluentResource {
    match FluentResource::try_new(source.ftl().to_owned()) {
        Ok(resource) => resource,
        Err((resource, errors)) => {
            for error in errors {
                let line = line_of(source.ftl(), error.pos.start);
                report(line, &error);
            }

            resource
        }
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text.as_bytes()[..offset]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// The key of an entry (a term keeps its leading `-`) and its name.
pub(crate) fn entry_id<'s>(entry: &ast::Entry<&'s str>) -> Option<(String, &'s str)> {
    match entry {
        ast::Entry::Message(m) => Some((m.id.name.to_owned(), m.id.name)),
        ast::Entry::Term(t) => Some((format!("-{}", t.id.name), t.id.name)),
        _ => None,
    }
}

/// The variables `pattern` picks a variant by, at any depth.
pub(crate) fn selectors<'s>(pattern: &ast::Pattern<&'s str>, out: &mut Vec<&'s str>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_selectors(expression, out);
        }
    }
}

fn expression_selectors<'s>(expression: &ast::Expression<&'s str>, out: &mut Vec<&'s str>) {
    match expression {
        ast::Expression::Select { selector, variants } => {
            if let ast::InlineExpression::VariableReference { id } = selector
                && !out.contains(&id.name)
            {
                out.push(id.name);
            }

            for variant in variants {
                selectors(&variant.value, out);
            }
        }
        ast::Expression::Inline(ast::InlineExpression::Placeable { expression }) => {
            expression_selectors(expression, out);
        }
        ast::Expression::Inline(_) => {}
    }
}

/// `args` with every selected variable that is a number spelled as a string
/// turned into that number, or `None` if there is none to turn.
fn numbered<'a>(args: &'a FluentArgs<'_>, selected: &[Box<str>]) -> Option<FluentArgs<'a>> {
    let is_selected = |name: &str| selected.iter().any(|s| **s == *name);

    let spelled = |name: &str, value: &FluentValue<'_>| {
        let FluentValue::String(text) = value else {
            return false;
        };

        is_selected(name) && matches!(FluentValue::try_number(text), FluentValue::Number(_))
    };

    if !args.iter().any(|(name, value)| spelled(name, value)) {
        return None;
    }

    let mut out = FluentArgs::with_capacity(args.iter().count());

    for (name, value) in args.iter() {
        let value = match value {
            FluentValue::String(text) if is_selected(name) => FluentValue::try_number(text),
            value => value.clone(),
        };

        out.set(name, value);
    }

    Some(out)
}
