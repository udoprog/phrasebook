//! Checks of a catalog, for an app's or a library's tests.
//!
//! ```
//! use phrasebook::{Catalog, Lang, Source, check};
//!
//! const SV_SE: Lang = Lang::new("sv-SE");
//!
//! static L10N: Catalog = Catalog::new("app", &[
//!     Source::new(Lang::EN_US, "hello = Hello\nitems = { $count ->\n    [one] One item\n   *[other] { $count } items\n}\n"),
//!     Source::new(SV_SE, "hello = Hej\nitems = { $count ->\n    [one] En sak\n   *[other] { $count } saker\n}\n"),
//! ]);
//!
//! check::catalog(&L10N).unwrap();
//! check::parity(&L10N, SV_SE).unwrap();
//! assert_eq!(check::ids(&L10N, SV_SE), ["hello", "items"]);
//! ```
//!
//! A failing check lists every problem it found:
//!
//! ```
//! use phrasebook::{Catalog, Lang, Source, check};
//!
//! static LIB: Catalog = Catalog::new("lib", &[
//!     Source::new(Lang::EN_US, "lib-a = A\nlib-a = Again\nb = B\n"),
//! ])
//! .with_prefix("lib-");
//!
//! let problems = check::catalog(&LIB).unwrap_err();
//! assert_eq!(problems.to_string(), "\
//! lib en-US: `lib-a` is defined twice
//! lib en-US: `b` does not start with the catalog's prefix `lib-`");
//! ```
//!
//! An app's test module usually needs only [`all`], naming the library
//! catalogs it translates, and [`Phrasebook::has_catalog`] for each of them,
//! plus [`coverage`] if it collects the ids its code asks for:
//!
//! ```
//! use phrasebook::{Catalog, Lang, Phrasebook, Source, check};
//!
//! # static LIB: Catalog = Catalog::new("lib", &[Source::new(Lang::EN_US, "lib-ok = OK\n")]).with_prefix("lib-");
//! static L10N: Catalog = Catalog::new("app", &[
//!     Source::new(Lang::EN_US, "title = App\n"),
//!     Source::new(Lang::new("sv-SE"), "title = App\nlib-ok = Okej\n"),
//! ]);
//!
//! fn phrasebook() -> Phrasebook {
//!     Phrasebook::builder()
//!         .language(Lang::EN_US, "English")
//!         .language(Lang::new("sv-SE"), "Svenska")
//!         .catalog(&L10N)
//!         .catalog(&LIB)
//!         .build()
//! }
//!
//! // In `#[test]` functions:
//! check::all(&L10N, &[&LIB]).unwrap();
//! assert!(phrasebook().has_catalog(&LIB));
//! ```
//!
//! [`Phrasebook::has_catalog`]: crate::Phrasebook::has_catalog

use std::collections::{HashMap, HashSet};
use std::fmt;

use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use fluent_syntax::ast;

use crate::phrasebook::{entry_id, new_bundle, parse};
use crate::{Catalog, Lang, Source};

/// Every source parses; no language has two sources; no id is defined
/// twice in a source; every id has the catalog's prefix; and every message
/// and attribute formats without errors, given a placeholder for every
/// variable the source mentions.
pub fn catalog(catalog: &Catalog) -> Result<(), Problems> {
    let mut problems = Problems::default();
    let mut langs = HashSet::new();

    for source in catalog.sources() {
        let lang = source.lang();
        let mut report = |message: String| problems.push(catalog, lang, message);

        if !langs.insert(lang) {
            report("a second source for this language, which is ignored".to_owned());
            continue;
        }

        check_source(catalog, source, &mut report);
    }

    problems.into_result()
}

fn check_source(catalog: &Catalog, source: &Source, report: &mut dyn FnMut(String)) {
    let resource = parse(source, |line, error| {
        report(format!("syntax error on line {line}: {error}"));
    });

    let mut seen = HashSet::new();

    for (key, name) in resource.entries().filter_map(entry_id) {
        if !seen.insert(key.clone()) {
            report(format!("`{key}` is defined twice"));
        }

        if !catalog.owns(name) {
            let prefix = catalog.prefix().unwrap_or_default();
            report(format!(
                "`{key}` does not start with the catalog's prefix `{prefix}`"
            ));
        }
    }

    // Placeholders for every variable the source mentions: message
    // references share their caller's arguments, so a message's own
    // variables are not enough.
    let mut variables = Vec::new();

    for entry in resource.entries() {
        if let ast::Entry::Message(message) = entry {
            if let Some(pattern) = &message.value {
                pattern_variables(pattern, &mut variables);
            }

            for attribute in &message.attributes {
                pattern_variables(&attribute.value, &mut variables);
            }
        }
    }

    let mut args = FluentArgs::new();

    for name in variables {
        args.set(name.to_owned(), FluentValue::from(1));
    }

    let ids = resource
        .entries()
        .filter_map(|entry| match entry {
            ast::Entry::Message(m) => Some(m.id.name.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut bundle = new_bundle(source.lang(), false);
    // Duplicates are reported above.
    let _ = bundle.add_resource(resource);

    let mut reported = HashSet::new();

    for id in ids {
        if !reported.insert(id.clone()) {
            continue;
        }

        let Some(message) = bundle.get_message(&id) else {
            continue;
        };

        let patterns = message
            .value()
            .map(|p| (None, p))
            .into_iter()
            .chain(message.attributes().map(|a| (Some(a.id()), a.value())));

        for (attribute, pattern) in patterns {
            let mut errors = Vec::new();
            let _ = bundle.format_pattern(pattern, Some(&args), &mut errors);

            let name = match attribute {
                Some(attribute) => format!("{id}.{attribute}"),
                None => id.clone(),
            };

            for error in errors {
                report(format!("`{name}` does not format: {error}"));
            }
        }
    }
}

/// `lang` defines exactly the message ids of the catalog's default
/// language (its first source). Terms are a translation's own business and
/// are not compared.
///
/// An app that translates a library's messages in its own catalog checks
/// it with [`parity_with`] instead.
pub fn parity(catalog: &Catalog, lang: Lang) -> Result<(), Problems> {
    parity_with(catalog, lang, &[])
}

/// As [`parity`], for a catalog that also translates the messages of
/// `libraries`: an id in `lang` that the catalog's default language lacks
/// is accepted when one of `libraries` defines it in that library's default
/// language. Library ids are never required in the catalog.
///
/// ```
/// use phrasebook::{Catalog, Lang, Source, check};
///
/// const SV_SE: Lang = Lang::new("sv-SE");
///
/// static LIB: Catalog = Catalog::new("lib", &[
///     Source::new(Lang::EN_US, "lib-sign-in = Sign in\nlib-sign-out = Sign out\n"),
/// ])
/// .with_prefix("lib-");
///
/// static APP: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, "title = Mafia\n"),
///     Source::new(SV_SE, "title = Maffia\nlib-sign-in = Logga in\nlib-sing-out = Logga ut\n"),
/// ]);
///
/// assert!(check::parity(&APP, SV_SE).is_err());
///
/// let problems = check::parity_with(&APP, SV_SE, &[&LIB]).unwrap_err();
/// assert_eq!(
///     problems.to_string(),
///     "app sv-SE: `lib-sing-out` is defined neither in en-US nor in any given library"
/// );
/// ```
pub fn parity_with(catalog: &Catalog, lang: Lang, libraries: &[&Catalog]) -> Result<(), Problems> {
    let mut problems = Problems::default();

    let Some(default) = catalog.default_lang() else {
        problems.push(catalog, lang, "the catalog has no sources".to_owned());
        return problems.into_result();
    };

    if catalog.source(lang).is_none() {
        problems.push(catalog, lang, "no source for this language".to_owned());
        return problems.into_result();
    }

    let expected = ids(catalog, default);
    let actual = ids(catalog, lang);

    let expected_set = expected.iter().collect::<HashSet<_>>();
    let actual_set = actual.iter().collect::<HashSet<_>>();

    let library_ids = libraries
        .iter()
        .filter_map(|library| Some(ids(library, library.default_lang()?)))
        .flatten()
        .collect::<HashSet<_>>();

    for id in &expected {
        if !actual_set.contains(id) {
            problems.push(
                catalog,
                lang,
                format!("`{id}` is missing (defined in {default})"),
            );
        }
    }

    for id in &actual {
        if expected_set.contains(id) || library_ids.contains(id) {
            continue;
        }

        let message = if libraries.is_empty() {
            format!("`{id}` is not defined in {default}")
        } else {
            format!("`{id}` is defined neither in {default} nor in any given library")
        };

        problems.push(catalog, lang, message);
    }

    problems.into_result()
}

/// Every check of a catalog in one report: [`catalog`], and
/// [`parity_with`] `libraries` for every language besides the default, so a
/// language added later is checked without touching the test.
///
/// ```
/// use phrasebook::{Catalog, Lang, Source, check};
///
/// const SV_SE: Lang = Lang::new("sv-SE");
///
/// static LIB: Catalog = Catalog::new("lib", &[
///     Source::new(Lang::EN_US, "lib-sign-in = Sign in\n"),
/// ])
/// .with_prefix("lib-");
///
/// static APP: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, "title = Mafia\nrules = Rules\n"),
///     Source::new(SV_SE, "title = Maffia\nlib-sign-in = Logga in\n"),
/// ]);
///
/// let problems = check::all(&APP, &[&LIB]).unwrap_err();
/// assert_eq!(problems.to_string(), "app sv-SE: `rules` is missing (defined in en-US)");
/// ```
pub fn all(catalog: &Catalog, libraries: &[&Catalog]) -> Result<(), Problems> {
    let mut problems = Problems::default();
    problems.extend(self::catalog(catalog));

    let default = catalog.default_lang();
    let mut seen = HashSet::new();

    for lang in catalog.languages() {
        if Some(lang) != default && seen.insert(lang) {
            problems.extend(parity_with(catalog, lang, libraries));
        }
    }

    problems.into_result()
}

/// The message ids a catalog defines in any language, each once: the
/// default language's in file order, then any other language's extras.
/// For a library's ids, which an app's own checks of coverage exempt.
///
/// ```
/// use phrasebook::{Catalog, Lang, Source, check};
///
/// static LIB: Catalog = Catalog::new("lib", &[
///     Source::new(Lang::EN_US, "a = A\nb = B\n"),
///     Source::new(Lang::new("sv-SE"), "b = B\nc = C\n"),
/// ]);
///
/// assert_eq!(check::all_ids(&LIB), ["a", "b", "c"]);
/// ```
pub fn all_ids(catalog: &Catalog) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for lang in catalog.languages() {
        for id in ids(catalog, lang) {
            if seen.insert(id.clone()) {
                out.push(id);
            }
        }
    }

    out
}

/// The ids the code asks for against the ids the catalog defines in `lang`:
/// an asked id that neither the catalog nor any of `libraries` defines is
/// missing, and a catalog id that nobody asked for is unused, unless one of
/// `libraries` defines it (the app translating or overriding a library's
/// message).
///
/// Finding the ids the code asks for is the app's business, such as a scan
/// of its sources for the ids it spells out; ids it builds at runtime are
/// passed in as well, or checked with [`defines`].
///
/// ```
/// use phrasebook::{Catalog, Lang, Source, check};
///
/// static LIB: Catalog = Catalog::new("lib", &[
///     Source::new(Lang::EN_US, "lib-sign-in = Sign in\n"),
/// ])
/// .with_prefix("lib-");
///
/// static APP: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, "title = App\nold = Old\nlib-sign-in = Log in\n"),
/// ]);
///
/// let asked = ["title", "subtitle", "lib-sign-in"];
/// let problems = check::coverage(&APP, Lang::EN_US, asked, &[&LIB]).unwrap_err();
/// assert_eq!(problems.to_string(), "\
/// app en-US: `subtitle` is asked for but not defined
/// app en-US: `old` is defined but never asked for");
/// ```
pub fn coverage<'a>(
    catalog: &Catalog,
    lang: Lang,
    asked: impl IntoIterator<Item = &'a str>,
    libraries: &[&Catalog],
) -> Result<(), Problems> {
    let mut problems = Problems::default();

    if catalog.source(lang).is_none() {
        problems.push(catalog, lang, "no source for this language".to_owned());
        return problems.into_result();
    }

    let defined = ids(catalog, lang);
    let defined_set = defined.iter().map(String::as_str).collect::<HashSet<_>>();
    let library_ids = libraries
        .iter()
        .flat_map(|library| all_ids(library))
        .collect::<HashSet<_>>();

    let mut asked_set = HashSet::new();

    for id in asked {
        if !asked_set.insert(id) {
            continue;
        }

        if !defined_set.contains(id) && !library_ids.contains(id) {
            problems.push(
                catalog,
                lang,
                format!("`{id}` is asked for but not defined"),
            );
        }
    }

    for id in &defined {
        if !asked_set.contains(id.as_str()) && !library_ids.contains(id) {
            problems.push(
                catalog,
                lang,
                format!("`{id}` is defined but never asked for"),
            );
        }
    }

    problems.into_result()
}

/// Every id in `ids` is defined in `lang` by the catalog or one of
/// `libraries`: the missing half of [`coverage`] alone, for ids the code
/// builds at runtime.
///
/// ```
/// use phrasebook::{Catalog, Lang, Source, check};
///
/// static APP: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, "role-mafia = Mafia\nrole-town = Town\n"),
/// ]);
///
/// let roles = ["mafia", "town", "doctor"].map(|role| format!("role-{role}"));
/// let problems = check::defines(&APP, Lang::EN_US, roles.iter().map(String::as_str), &[])
///     .unwrap_err();
/// assert_eq!(problems.to_string(), "app en-US: `role-doctor` is asked for but not defined");
/// ```
pub fn defines<'a>(
    catalog: &Catalog,
    lang: Lang,
    ids: impl IntoIterator<Item = &'a str>,
    libraries: &[&Catalog],
) -> Result<(), Problems> {
    let mut problems = Problems::default();
    let defined = self::ids(catalog, lang);
    let defined = defined.iter().map(String::as_str).collect::<HashSet<_>>();
    let library_ids = libraries
        .iter()
        .flat_map(|library| all_ids(library))
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();

    for id in ids {
        if seen.insert(id) && !defined.contains(id) && !library_ids.contains(id) {
            problems.push(
                catalog,
                lang,
                format!("`{id}` is asked for but not defined"),
            );
        }
    }

    problems.into_result()
}

/// The message ids a catalog defines in `lang`, in file order, each once.
/// Empty if the catalog has no source for `lang`. Whatever parses is
/// listed, so a source with syntax errors still lists the rest.
pub fn ids(catalog: &Catalog, lang: Lang) -> Vec<String> {
    let Some(source) = catalog.source(lang) else {
        return Vec::new();
    };

    let resource: FluentResource = parse(source, |_, _| {});
    let mut seen = HashMap::new();
    let mut out = Vec::new();

    for entry in resource.entries() {
        if let ast::Entry::Message(m) = entry
            && seen.insert(m.id.name, ()).is_none()
        {
            out.push(m.id.name.to_owned());
        }
    }

    out
}

fn pattern_variables<'s>(pattern: &ast::Pattern<&'s str>, out: &mut Vec<&'s str>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_variables(expression, out);
        }
    }
}

fn expression_variables<'s>(expression: &ast::Expression<&'s str>, out: &mut Vec<&'s str>) {
    match expression {
        ast::Expression::Select { selector, variants } => {
            inline_variables(selector, out);

            for variant in variants {
                pattern_variables(&variant.value, out);
            }
        }
        ast::Expression::Inline(inline) => inline_variables(inline, out),
    }
}

fn inline_variables<'s>(inline: &ast::InlineExpression<&'s str>, out: &mut Vec<&'s str>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            if !out.contains(&id.name) {
                out.push(id.name);
            }
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                inline_variables(argument, out);
            }

            for argument in &arguments.named {
                inline_variables(&argument.value, out);
            }
        }
        ast::InlineExpression::Placeable { expression } => expression_variables(expression, out),
        // A term's arguments are its own scope.
        _ => {}
    }
}

/// The problems a check found, one per line when displayed.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct Problems {
    problems: Vec<Problem>,
}

impl Problems {
    fn push(&mut self, catalog: &Catalog, lang: Lang, message: String) {
        self.problems.push(Problem {
            catalog: catalog.name().to_owned(),
            lang,
            message,
        });
    }

    fn extend(&mut self, result: Result<(), Problems>) {
        if let Err(other) = result {
            self.problems.extend(other.problems);
        }
    }

    fn into_result(self) -> Result<(), Problems> {
        if self.problems.is_empty() {
            Ok(())
        } else {
            Err(self)
        }
    }

    /// The problems, in the order found.
    pub fn iter(&self) -> impl Iterator<Item = &Problem> {
        self.problems.iter()
    }
}

impl<'a> IntoIterator for &'a Problems {
    type Item = &'a Problem;
    type IntoIter = std::slice::Iter<'a, Problem>;

    fn into_iter(self) -> Self::IntoIter {
        self.problems.iter()
    }
}

impl fmt::Display for Problems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (n, problem) in self.problems.iter().enumerate() {
            if n > 0 {
                writeln!(f)?;
            }

            problem.fmt(f)?;
        }

        Ok(())
    }
}

/// Lists the problems as [`Display`](fmt::Display) does, so a failed
/// `unwrap()` in a test reads well.
impl fmt::Debug for Problems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        fmt::Display::fmt(self, f)
    }
}

impl core::error::Error for Problems {}

/// One problem a check found, in one catalog and language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    catalog: String,
    lang: Lang,
    message: String,
}

impl Problem {
    /// The name of the catalog it is in.
    pub fn catalog(&self) -> &str {
        &self.catalog
    }

    /// The language of the source it is in.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// What is wrong.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.catalog, self.lang, self.message)
    }
}
