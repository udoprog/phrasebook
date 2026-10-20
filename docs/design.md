# phrasebook design

phrasebook is the Fluent localization layer of a Rust web frontend: one
Fluent bundle per supported language, built from `.ftl` resources compiled
into the binary; message lookup with arguments and plurals; a fallback
language and visible missing-message behaviour; the user's chosen language
remembered in the browser; and a handle, `Phrases`, that is itself the Yew
context type (with yew glue behind a versioned `yew023` feature), through
which any component looks messages up, including components of a library such
as watchword, which ship their own messages and fall back to built-in English
when the app provides nothing.

It replaces three hand-rolled `l10n.rs` modules:

- `~/repo/livemix/crates/web/src/l10n.rs` (642 lines),
- `~/repo/mafia/crates/frontend/src/l10n.rs` (261 lines),
- `~/repo/army/crates/web/src/l10n.rs` (121 lines).

This document records:

1. a survey of those modules, their catalogs, each app's `Locale` type and
   language choice, and how components call in, with where they differ;
2. the API: layout and features, then each part;
3. how livemix, mafia and army each wire it up and what each deletes;
4. the decisions behind the API's shape, and the questions left open.

The API sketches in section 2 match the crate as built. The crate docs (`src/lib.rs` and the items' own docs) are
authoritative for the details, written in terms of behaviour rather than of
the apps it came from; this document explains why the API is shaped as it is.

## 1. Survey

### 1.1 The three modules

All three are a context handle named `L10n` holding an `Rc` to parsed Fluent
bundles, compared by pointer (plus language, where there is one), so handing it
down a component tree re-renders only when the language changes. All three
embed their `.ftl` with `include_str!`, keep whatever parsed when a resource has
errors, call `set_use_isolating(false)` because the text lands in HTML text
nodes, and render a missing message as its own id so the hole is visible.

**mafia** (`crates/frontend/src/l10n.rs`, 261 lines; en-US and sv-SE):

- `L10n { inner: Rc<Inner { en, sv }>, lang: Locale }`, one field per language,
  matched by hand in `bundle()` and `langid()`.
- `text(key) -> String` and `msg(key, &FluentArgs) -> String`, plus a free
  `args([(name, value)])` building `FluentArgs` from same-typed pairs.
- Lookup: current language, then en-US, then the key. Silent: no logging of a
  missing message, a parse error or a format error.
- `detect(navigator.language)` matches the primary subtag (`sv`, `en`) by hand,
  `load()` reads localStorage key `"lang"` (value `en-US`/`sv-SE`), then the
  browser, then `Locale::DEFAULT`; `store(locale)` writes it.
- `with_lang(lang)` gives a handle onto the same bundles in another language.
- Tests: every en-US key resolves; sv-SE defines exactly the en-US keys (parity)
  and resolves each without fallback; placeables interpolate; detection.

**livemix** (`crates/web/src/l10n.rs`, 642 lines; en-US only so far, about 260
lines of code and 380 of tests):

- The same shape as mafia (it is the same code, grown), with `Inner { en,
  resolved }`; `bundle()` and `langid()` ignore the language because only en-US
  ships.
- `text(key) -> AttrValue`, resolved once per language and key and cached in
  `resolved: RefCell<HashMap<Locale, HashMap<Box<str>, AttrValue>>>`, so the
  1,516 `text` calls hand out `Rc` clones after the first render. `msg(key,
  &FluentArgs) -> String` is not cached.
- **Selector numbers**: a variant key such as `[one]` or `[0]` only matches a
  number, so a count passed as its digits always falls through to `*[other]`.
  `format_in` walks the message's AST (`fluent-syntax`) for the variables it
  selects on, at any depth, and re-parses any of those passed as a numeric
  string into a number. Variables not selected on stay as written (`"01"`
  stays `"01"`). This is redone on every `msg` call.
- `detect`, `load`, `store` and the `"lang"` key as in mafia.
- Components reach it through livemix's generic `ctx::use_ctx::<L10n>()` and
  `ctx::link_ctx(link, cb)` (47 `use_ctx::<L10n>()` sites; `link_ctx` has 188
  across all context types),
  whose `ContextFallback` builds `L10n::new(Locale::DEFAULT)` once per thread
  when no provider is present and warns once.
- Tests worth keeping somewhere: the catalog parses with unique ids; every key
  resolves; a selected count reads the same as a number and as digits, for
  every selecting message; unselected digits stay as written; placeables and
  plurals (`midi-orphaned`: "One mapping", "2 mappings"). The rest are
  livemix's own vocabulary rules (ASCII punctuation, Sends/Feeds wording,
  `-detail` keys having their line, keys named by the `api` crate resolving)
  and the `sweep` crate's two-way check that every key asked for exists and
  every key defined is asked for, which finds asks by grepping for
  `l10n.text("` and `l10n.msg("`.

**army** (`crates/web/src/l10n.rs`, 121 lines; en-US only, no language choice):

- `L10n { bundle: Rc<FluentBundle> }`, no language, no fallback chain.
- `get(key) -> String` and `args(key, [(name, value)]) -> String`, the pairs
  same-typed, so mixed-type calls stringify numbers
  (`[("pts", number(points)), ("limit", limit.to_string())]`), the exact case
  livemix's selector numbers exist for.
- The only one that logs: `tracing::warn!` on a missing message and on format
  errors, `tracing::error!` on syntax and resource errors.
- No storage or detection.
- Struct components use `armies::l10n(ctx)`, which on a missing provider
  returns `L10n::new()`, re-parsing the whole catalog on every call.
- Root builds its own `L10n` (`use_memo`) to pass `title` and
  `signed_out_notice` to watchword's `AuthProvider`, and `App` holds another,
  providing it with `ContextProvider<L10n>`.

### 1.2 Catalogs

| | livemix | mafia | army |
|---|---|---|---|
| Files | `en-US.ftl` | `en-US.ftl`, `sv-SE.ftl` | `en-US.ftl` |
| Messages | 2,388 (3,256 lines) | 278 each | 268 |
| Select expressions | 24 (`[one]`, `[0]`, `*[other]`) | none | 3 (`[one]`) |
| Terms, attributes, functions, message references | none | none | none |

So the Fluent in use is messages with variables and plural or numeric
selectors. The design supports all of Fluent (it is Fluent underneath) but has
to make this subset work well.

### 1.3 Locale types and language choice

- **livemix** `api::Locale` is a struct of `api::Language` and `api::Country`
  (each a `[u8; 4]` newtype over the `iso639`/`iso3166` tables, the code koine
  is being extracted from). It has `EN_US`, `ALL`, `DEFAULT`, `from_code`,
  `from_iso`, `label()` (the endonym, "English") and `flag()` (an ISO 3166
  alpha-2 code with a vendored flag, else `"UN"`). The toolbar's
  `LanguageSelector` lists `Locale::ALL` with flag and label and emits
  `Msg::SetLang`, which `App` handles with `with_lang` and `store`.
- **mafia** `api::Locale` is an enum `EnUs | SvSe` with `ALL`, `DEFAULT`,
  `code`, `label` and `from_code`, shared with the server: it is musli-encoded
  on the wire and stored in SQLite (`sqll`), because role and vote texts are
  translated in the database and travel keyed by it. About 25 components call
  `strings.get(api::Field::Name, l10n.lang())` to pick those texts. `App` owns
  the `L10n`, uses it for the tab title (`title_for_route`) above its own
  provider, handles `Msg::SetLang` with `with_lang` and `store`, and on
  projector pages cycles through `Locale::ALL` on a timer (`Msg::CycleLang`)
  without storing.
- **army** has no `Locale`.

So each app's locale type does more than pick a bundle (wire format, database
column, flags), and stays the app's. phrasebook needs its own small language
identifier that apps convert to and from.

### 1.4 How components call in

| | livemix | mafia | army |
|---|---|---|---|
| Plain text | `l10n.text(` x1,516 | `l10n.text(` x295 | `l10n.get(` x230 |
| With arguments | `l10n.msg(` x207 | `l10n.msg(` x30 | `l10n.args(` x74 |
| Function components | `use_ctx::<L10n>()` | `use_context::<L10n>().expect(..)` | `use_context::<L10n>()` |
| Struct components | `link_ctx(link, cb)` | `.context::<L10n>(cb).expect(..)` | `armies::l10n(ctx)` |
| No provider | cached fallback, warn once | panic | re-parse per call |

### 1.5 Where they differ, and what phrasebook takes

| Concern | livemix | mafia | army | phrasebook |
|---|---|---|---|---|
| Languages | 1, hand-matched | 2, hand-matched | 1 | declared list, one bundle each |
| Fallback | en-US, then id | en-US, then id | id | declared fallback, then a library's built-in English, then id |
| Logging | none | none | `tracing` | `tracing`, once per message and language |
| Plain text type | cached `AttrValue` | `String` | `String` | cached `Rc<str>` |
| Arguments | `&FluentArgs`, same-typed `args()` | same | same-typed pairs | own `Args`, mixed types |
| Digits in selectors | read as numbers | no | no | read as numbers |
| Remembered language | localStorage `lang` | same | none | same key and value format |
| Detection | `navigator.language`, primary subtag | same | none | `navigator.languages`, exact then primary subtag |
| Missing provider | cached fallback, warn | panic | re-parse | cached fallback, warn once |
| Library messages | n/a | n/a | passed to watchword as props | library catalogs with prefixed ids |

## 2. API

### 2.1 Crates and features

One crate, **phrasebook** (decided by the user, questions 3 and 5): the
language identifier, catalogs, the dictionary, the `Phrases` handle,
arguments, the browser helpers, and, behind the `yew023` feature, the
`phrasebook::yew023` module for yew 0.23.

phrasebook ships no components and no hooks. The context type is the handle
itself, so an app provides it with yew's own `ContextProvider<Phrases>` and a
component reads it with yew's own `use_context::<Phrases>()`, or
`ctx.link().context::<Phrases>(callback)` in a struct component. What
`yew023` adds is trait impls on `Phrases` (and `Lang` and `Language`) and two plain
functions for when no handle is provided (2.10).

None of that needs yew's macros (`html!`, `#[function_component]`,
`#[derive(Properties)]`, `#[hook]`), which expand to `::yew::` paths and
would force yew to be a direct dependency under the key `yew`; watchword
splits watchword-yew023 out for that reason. phrasebook depends on yew under
the versioned key `yew023` (`yew023 = { package = "yew", version = "0.23",
optional = true }`) and so stays one crate. A later yew is another module and
feature beside it (`yew024`).

`std` is on by default; every other feature enables it. `alloc`, which
`std` enables, adds nothing yet: the two exist so that a later
alloc-only build is not a breaking change.

| Feature | Enables | Pulls in |
|---|---|---|
| *(none)* | `Lang`, `Catalog`, `Source` | nothing |
| `std` (default) | `Phrasebook`, `Phrases`, `Args`, `Value`, `args!`, the `check` test helpers | fluent-bundle, fluent-syntax, unic-langid, tracing (with its `std`) |
| `alloc` | nothing yet | nothing |
| `browser` | `phrasebook::browser`: remembered language in localStorage, browser preferences; `Phrases::store`; numbers formatted through `Intl.NumberFormat` | web-sys, js-sys, wasm-bindgen, intl-memoizer (on wasm32 only) |
| `koine01` | `phrasebook::koine01`: conversions between `Lang` and koine 0.1's `Language`, `Country` and `Locale`, the unknown-language warning, `Builder::language_named_by_koine` | koine 0.1 (as `koine01`, feature `locale`) |
| `yew023` | `phrasebook::yew023`: trait impls on `Phrases`, `Lang` and `Language`, the no-provider helpers | yew 0.23 (as `yew023`) |

**Versioned dependencies.** Per the user's convention, a pre-1.0 dependency
that appears in the public API is named after its version. fluent (0.17,
fluent-bundle 0.16) and unic-langid (0.9) are not 1.0 and are kept out of the
API entirely: `Args`, `Value` and `Lang` are phrasebook's own types, so a
fluent upgrade is an internal change rather than a `fluent017` feature. yew is
in the API (its traits, implemented on `Phrases`), hence `yew023`. koine is
not 1.0 and its types appear in the API, hence `koine01` (see 2.8). web-sys appears nowhere in the API, so
`browser` is unversioned. tracing is an internal dependency.

### 2.2 `Lang`: a language identifier

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lang { /* language subtag, optional region subtag, inline */ }

impl Lang {
    pub const EN_US: Lang;
    /// For constants: `const SV_SE: Lang = Lang::new("sv-SE");`. Panics (a
    /// compile error in a const) unless `tag` is `ll[-RR]` or `lll[-RR]`.
    pub const fn new(tag: &str) -> Lang;
    /// Case-insensitive, `-` or `_`, ignoring script and variant subtags, so
    /// it accepts what browsers report (`sv`, `en_GB`, `zh-Hant-TW`).
    pub fn parse(tag: &str) -> Option<Lang>;
    pub fn language(&self) -> &str;          // "sv"
    pub fn region(&self) -> Option<&str>;    // Some("SE")
    pub fn primary(&self) -> Lang;           // "sv"
}
// Display and FromStr: canonical "sv-SE"; Debug the same.
```

It is the shape of tag all three apps use (`language-REGION`), Copy and
comparable like their `Locale`s, and canonicalizes to the string they already
store (`en-US`). It is phrasebook's own type and not `koine::Locale`, although
koine now has that compact identifier, because `koine::Locale` is validated
against koine's tables: using it means linking koine's language table (about
106k generated lines), which every app would carry in its wasm for a check its
own declaration already makes. So the core checks only the shape, and koine is
behind `koine01`, where `Lang` converts to and from koine's `Locale`,
`Language` and `Country` (see 2.8 and question 2).

### 2.3 Catalogs: an app's or a library's messages

```rust
/// One language's `.ftl` source, embedded in the binary.
pub struct Source { /* lang, ftl */ }
impl Source {
    pub const fn new(lang: Lang, ftl: &'static str) -> Source;
    pub const fn lang(&self) -> Lang;
    pub const fn ftl(&self) -> &'static str;
}

/// Everything one crate ships: its name, the prefix every message id it
/// defines must start with (if any), and one source per language.
pub struct Catalog { /* name, prefix, sources */ }
impl Catalog {
    pub const fn new(name: &'static str, sources: &'static [Source]) -> Catalog;
    pub const fn with_prefix(self, prefix: &'static str) -> Catalog;
    pub const fn name(&self) -> &'static str;
    pub const fn prefix(&self) -> Option<&'static str>;
    pub const fn sources(&self) -> &'static [Source];
    /// The first source's language.
    pub fn default_lang(&self) -> Option<Lang>;
    pub fn languages(&self) -> impl Iterator<Item = Lang>;
    pub fn source(&self, lang: Lang) -> Option<&'static Source>;
}
```

An app declares its catalog as a static:

```rust
pub(crate) static L10N: Catalog = Catalog::new("mafia", &[
    Source::new(Lang::EN_US, include_str!("../locales/en-US.ftl")),
    Source::new(SV_SE, include_str!("../locales/sv-SE.ftl")),
]);
```

and a library with its prefix:

```rust
pub static L10N: Catalog = Catalog::new("watchword", &[
    Source::new(Lang::EN_US, include_str!("../locales/en-US.ftl")),
])
.with_prefix("watchword-");
```

The first source is the catalog's own default language, which is what it
falls back to when used without a provider (2.10). Sources stay `&'static str`
and are parsed only when a bundle for that language is first needed.

### 2.4 `Phrasebook`: the dictionary

```rust
#[derive(Clone)]
pub struct Phrasebook { /* Rc<Inner> */ }

impl Phrasebook {
    pub fn builder() -> Builder;
    /// A handle in `lang`, or in the fallback if `lang` is not declared.
    pub fn phrases(&self, lang: Lang) -> Phrases;
    pub fn languages(&self) -> &[Language];
    pub fn fallback(&self) -> Lang;
    /// Whether `lang` is declared.
    pub fn supports(&self, lang: Lang) -> bool;
    /// The best declared language for a list of preferred tags, most
    /// preferred first: an exact match, then the first declared language
    /// with the same primary subtag, then the fallback.
    pub fn negotiate<'a>(&self, preferred: impl IntoIterator<Item = &'a str>) -> Lang;
}

/// A supported language and its name in itself, for a language picker.
#[non_exhaustive]
pub struct Language { pub lang: Lang, pub name: &'static str }
impl Language { pub const fn new(lang: Lang, name: &'static str) -> Language; }

impl Builder {
    /// Declare a supported language, in picker order. The first is the
    /// fallback unless `fallback` says otherwise.
    pub fn language(self, lang: Lang, name: &'static str) -> Self;
    /// Each of a list, in order, as `language`.
    pub fn languages(self, languages: impl IntoIterator<Item = (Lang, &'static str)>) -> Self;
    pub fn fallback(self, lang: Lang) -> Self;
    /// Add a catalog. Earlier catalogs win a duplicate id, so the app adds
    /// its own first and can override a library's message (see 4.4).
    pub fn catalog(self, catalog: &'static Catalog) -> Self;
    /// Bidi isolation marks around placeables; off by default (4.6).
    pub fn use_isolating(self, on: bool) -> Self;
    /// With `browser` in the browser, numbers through `Intl.NumberFormat`
    /// in the bundle's language (a fluent formatter). On by default.
    pub fn locale_numbers(self, on: bool) -> Self;
    pub fn build(self) -> Phrasebook;
}
```

The app declares its languages in one place, the builder, with their
endonyms. A catalog source for an undeclared language is ignored; a declared
language with no source in some catalog falls back for that catalog's
messages. Each declared language gets one Fluent bundle holding every
catalog's resource for it, built and cached on first use, so an app shipping
five languages parses one at start-up. A duplicate id across catalogs, an id
outside its catalog's prefix, and a syntax error are logged with `tracing`
when the bundle is built; `check` (2.9) turns them into test failures.

### 2.5 `Phrases`: the handle components use

`Phrasebook` is the shared set of every language's bundles; `Phrases` is the
phrasebook open at the current language, and is what components get from
context: `use_context::<Phrases>()`, then `phrases.text(...)` and
`phrases.msg(...)`.

```rust
#[derive(Clone)]
pub struct Phrases { /* Phrasebook, Lang, Option<&'static Catalog> backstop */ }
// PartialEq: same Phrasebook (pointer), same language, same backstop.

impl Phrases {
    /// A shared, per-thread handle with no catalogs: every lookup renders
    /// the id.
    pub fn empty() -> Phrases;
    /// A catalog alone, in its own default language, built once per thread
    /// per catalog; what a library renders when nothing is provided.
    pub fn from_catalog(catalog: &'static Catalog) -> Phrases;
    /// The same handle, with `catalog` consulted after every bundle in the
    /// chain lacks a message (see Lookup).
    pub fn with_backstop(&self, catalog: &'static Catalog) -> Phrases;

    pub fn lang(&self) -> Lang;
    pub fn languages(&self) -> &[Language];
    pub fn phrasebook(&self) -> &Phrasebook;
    /// The same bundles in another language; an app's root calls this on a
    /// language change and provides the result.
    pub fn with_lang(&self, lang: Lang) -> Phrases;

    /// A message without arguments, resolved once per language and id and
    /// handed out as an `Rc` clone after that. Converts into yew's
    /// `AttrValue` and into children without copying.
    pub fn text(&self, id: &str) -> Rc<str>;
    /// A message with arguments.
    pub fn msg(&self, id: &str, args: &Args<'_>) -> String;
    /// The same, without the fallback to the id: `None` when no language in
    /// the chain defines it.
    pub fn try_text(&self, id: &str) -> Option<Rc<str>>;
    pub fn try_msg(&self, id: &str, args: &Args<'_>) -> Option<String>;
}
```

The handle carries no way to switch language. Switching is the app's state:
its root calls `with_lang` (and `store`, 2.7) and provides the new handle. A
deep component that changes the language (a picker) gets a callback from the
app, as it would for any other app state.

`text` and `msg` keep the names livemix and mafia use, so their call sites
carry over with at most a variable rename (`l10n` to `phrases`); army's `get`
and `args` are renamed (section 3).

**Lookup.** A message is formatted in the first of these that defines it:
the handle's language; the fallback language; the handle's backstop catalog,
if it has one (`with_backstop`, used by `or_catalog` in 2.10), in the
handle's language when the catalog has a source for it and in the catalog's
default language otherwise; and otherwise
the result is the id itself, so the hole is
visible on the page rather than blank. Each miss (the handle's language
lacking a message, or every bundle lacking it) is logged once per id and
language with `tracing::warn!`, and format errors (a missing argument, say)
once per id; a page re-renders often, and a warning per render would drown
the console.

**Selector numbers.** A variable a message selects on (`{ $count -> ...}`, at
any depth) that is passed as a string spelling a number is formatted as that
number, so `[one]` and `[0]` match whether the caller passed `3` or `"3"`.
Other variables are left as written. This is livemix's rule; phrasebook works
out each message's selector variables once, when it is first formatted, rather
than walking the AST on every call (question 4).

### 2.6 Arguments

```rust
pub struct Args<'a> { /* FluentArgs<'a> */ }
impl<'a> Args<'a> {
    pub fn new() -> Self;
    pub fn set(&mut self, name: &'a str, value: impl Into<Value<'a>>) -> &mut Self;
    pub fn with(self, name: &'a str, value: impl Into<Value<'a>>) -> Self;
}
impl<'a, V: Into<Value<'a>>> FromIterator<(&'a str, V)> for Args<'a>;

/// Same-typed pairs, as the apps' `args()` today: `args([("count", n)])`.
pub fn args<'a, V: Into<Value<'a>>>(pairs: impl IntoIterator<Item = (&'a str, V)>) -> Args<'a>;

/// Mixed types: `args!(count = 3, name = &name)`.
macro_rules! args { ... }

pub struct Value<'a> { /* FluentValue<'a> */ }
// From: every integer and float type, &str, String, Cow<str>, Rc<str>.
```

`args([...])` keeps today's call sites compiling; `args!` covers the
mixed-type calls that currently stringify numbers.

### 2.7 Remembering the language (`browser`)

```rust
pub mod browser {
    /// localStorage key the apps already use.
    pub const DEFAULT_KEY: &str = "lang";
    /// The remembered choice, if it parses and is declared.
    pub fn remembered(book: &Phrasebook, key: &str) -> Option<Lang>;
    /// `navigator.languages`, most preferred first.
    pub fn preferred() -> Vec<String>;
    /// The handle to start with, open at the choice remembered under
    /// `DEFAULT_KEY`, else the browser's preferences negotiated against the
    /// declared languages, else the fallback; marks the page as in it.
    pub fn initial(book: &Phrasebook) -> Phrases;
    /// The same, with the choice remembered under another key.
    pub fn initial_with_key(book: &Phrasebook, key: &str) -> Phrases;
    /// The language `initial_with_key` opens its handle at.
    pub fn initial_lang(book: &Phrasebook, key: &str) -> Lang;
    /// Set `<html lang>`, for screen readers and hyphenation.
    pub fn set_document_lang(lang: Lang);
}

impl Phrases {
    /// Remember the handle's language under `browser::DEFAULT_KEY`.
    pub fn store(&self);
    /// The same, under another key.
    pub fn store_as(&self, key: &str);
}
```

An app's root calls `browser::initial` once for its starting handle,
`browser::initial(&book)`, which also sets the page's `<html lang>`, and on
a language the user picked, `phrases.with_lang(lang)` followed by
`browser::set_document_lang(lang)` and `store()` on the new handle. A change
the user did not choose (a projector page cycling through languages) skips
`store()`.

The key and value format (`"lang"`, `"en-US"`) are the apps' current ones, so
users keep their choice across the switch. Detection reads the whole
`navigator.languages` list rather than `navigator.language`, so a user
preferring `de, sv` gets Swedish from mafia instead of English. Outside wasm
(an app's host-side tests and clippy) these return nothing and store nothing.

### 2.8 koine (`koine01`)

koine (`~/repo/koine`, 0.1) has `Copy` value types over generated ISO
tables: `koine::Language` (an ISO 639-3 code, four bytes), `koine::Country`
(an ISO 3166-1 alpha-2 code, four bytes) and `koine::Locale` (a language and
an optional country, eight bytes), each with a `DEFAULT` sentinel that names
nothing, and the tables behind them (`language::by_part1`, `by_id`,
`country::by_alpha2`, `locale::contains`, each entry with its codes and
English `name`). phrasebook depends on it as `koine01` with
`default-features = false, features = ["locale"]` (which implies `language`
and `country`), so a `Lang` converts to and from the compact types rather
than the records the first draft of this section planned for.

```rust
pub mod koine01 {
    // Lang <-> koine; fails for an unknown language or region, and for a
    // DEFAULT sentinel.
    impl TryFrom<Lang> for koine::Locale { type Error = Error; }
    impl TryFrom<koine::Locale> for Lang { type Error = Error; }
    impl TryFrom<koine::Language> for Lang { type Error = Error; } // no region

    pub fn locale(lang: Lang) -> Option<koine::Locale>;   // the TryFrom, as Option
    pub fn lang(locale: koine::Locale) -> Option<Lang>;   // the TryFrom, as Option
    pub fn language(lang: Lang) -> Option<koine::Language>;
    pub fn country(lang: Lang) -> Option<koine::Country>;  // None without a region
    /// The language is in koine's table and, with a region, koine lists the
    /// pair as a valid locale (`Locale::is_listed`).
    pub fn is_known(lang: Lang) -> bool;
    /// koine's English reference name of the language ("Swedish").
    pub fn name(lang: Lang) -> Option<&'static str>;
    pub struct Error; // Display, std::error::Error
}
```

A `Lang` from koine spells its language as koine displays a `Locale`: the
ISO 639-1 code where there is one, the ISO 639-3 code otherwise (`swe`
becomes `sv`, `fil` stays `fil`), so `Lang` to `Locale` and back is the
identity for every canonical tag koine knows. The conversions are trait impls
on koine's types (allowed, since `Lang` is phrasebook's) and plain functions,
not `From`, because neither direction is total.

With `koine01` on, `Builder::build` warns about a declared language that is
not `is_known`, and `Builder::language_named_by_koine(lang)` declares a
language under koine's English name when an app has no endonym to hand. The
name leaves out the region (`en-US` and `en-GB` are both "English"), and a
language koine does not know is named by its tag, leaked once per call.

### 2.9 Checks for an app's or library's tests

```rust
pub mod check {
    /// Every source parses; no id is defined twice; every id has the
    /// catalog's prefix; every message formats without errors given
    /// placeholder arguments.
    pub fn catalog(catalog: &Catalog) -> Result<(), Problems>;
    /// `lang` defines exactly the ids of the catalog's default language.
    pub fn parity(catalog: &Catalog, lang: Lang) -> Result<(), Problems>;
    /// As `parity`, but an id in `lang` the default language lacks is
    /// accepted when one of `libraries` defines it in its default language.
    pub fn parity_with(
        catalog: &Catalog,
        lang: Lang,
        libraries: &[&Catalog],
    ) -> Result<(), Problems>;
    /// The message ids a catalog defines in one language, in file order.
    pub fn ids(catalog: &Catalog, lang: Lang) -> Vec<String>;

    /// Every problem found, each naming its catalog and language; `Display`
    /// lists them one per line, so `check::catalog(&L10N).unwrap()` fails a
    /// test with all of them.
    pub struct Problems;
    pub struct Problem; // catalog(), lang(), message()
}
```

These replace each app's "the catalog parses, every key resolves" tests and
mafia's sv-SE parity test. livemix's vocabulary rules stay in livemix, built
on `ids` and a `Phrases`.

An app that translates a library's messages in its own catalog (the
`watchword-*` ids in an app's `sv-SE.ftl`, section 2.11) checks parity with
`check::parity_with(&APP, SV_SE, &[&auth::yew023::L10N])`: those ids
pass because the library defines them, while a misspelled one, defined
neither in the app's default language nor in any given library, is still
reported. Library ids are never required in the app.

### 2.10 Yew (`yew023`)

phrasebook ships no components and no hooks. The context type is `Phrases`
itself, so an app provides it with yew's own `ContextProvider<Phrases>` (army
already does this with its `L10n`) and components read it with yew's own
`use_context::<Phrases>()`, or `ctx.link().context::<Phrases>(callback)` in a
struct component. The feature adds only this:

```rust
pub mod yew023 {
    // So the handle and language pass as props and context like yew's own
    // cheap values.
    impl ImplicitClone for Phrases {}
    impl ImplicitClone for Lang {}
    impl ImplicitClone for Language {}
    // For an element's `lang` attribute. `text`'s `Rc<str>` already
    // converts into `AttrValue` and into children.
    impl IntoPropValue<AttrValue> for Lang {}
    impl IntoPropValue<Option<AttrValue>> for Lang {}

    /// The provided handle, or `Phrases::empty()` (renders ids) with one
    /// warning per thread.
    pub fn or_default(phrases: Option<Phrases>) -> Phrases;

    /// For a library: the provided handle with the library's catalog as its
    /// backstop (`with_backstop`), so a message the app's bundles lack (the
    /// app did not add the catalog, or not in this language) comes from the
    /// library's default language; or, with no handle provided,
    /// `Phrases::from_catalog(catalog)`, the library's catalog alone in its
    /// default language.
    pub fn or_catalog(phrases: Option<Phrases>, catalog: &'static Catalog) -> Phrases;

    /// The message with `element` where the translation places `name`
    /// (`Phrases::msg_around`); appended if the translation lacks it.
    pub fn html_around(phrases: &Phrases, id: &str, args: &Args, name: &str, element: Html) -> Html;
}
```

Both are plain functions over the `Option<Phrases>` that `use_context`
returns, wrapping the core constructors in 2.5:

```rust
// A function component.
let phrases = or_default(use_context::<Phrases>());
// A library's function component.
let phrases = or_catalog(use_context::<Phrases>(), &L10N);

// A struct component, in `create`: map the context to its first element and
// keep the `ContextHandle` to stay subscribed; `Msg::Phrases(Phrases)`
// receives later handles.
let context = ctx.link().context::<Phrases>(ctx.link().callback(Msg::Phrases));
let phrases = or_default(context.as_ref().map(|(phrases, _)| phrases.clone()));
let handle = context.map(|(_, handle)| handle); // stored in the component
```

The app's root owns the handle, as with any other app state:

```rust
// create
let phrases = browser::initial(&l10n::phrasebook());
// update, on a language the user picked
self.phrases = self.phrases.with_lang(lang);
self.phrases.store();
// view
html! { <ContextProvider<Phrases> context={self.phrases.clone()}> ... </ContextProvider<Phrases>> }
```

phrasebook ships no language picker: the apps' pickers differ in looks (flags,
a context menu, a toolbar select) and need only `phrases.languages()`,
`phrases.lang()` and a `Callback<Lang>` from the app.

### 2.11 Library components (watchword)

watchword-yew023 has about a hundred English strings inline (`"Sign in"`,
`"Delete {name}"`, ...) and takes a few as props (`AuthProvider`'s `title` and
`signed_out_notice`, default `SESSION_ENDED`). With phrasebook it:

1. adds `crates/watchword-yew023/locales/en-US.ftl` with every string under
   the `watchword-` prefix (`watchword-sign-in`, `watchword-delete = Delete {
   $name }`), and exports `pub static L10N: phrasebook::Catalog` from
   `watchword::yew023`;
2. calls `or_catalog(use_context::<Phrases>(), &L10N)` in each function
   component (in a struct component, the same over `ctx.link().context`
   mapped to its first element) and looks its strings up by id;
3. keeps the props as overrides, defaulting to its own messages, so an app
   passing them today is unaffected;
4. tests its catalog with `check::catalog(&L10N)`, which also enforces the
   prefix.

An app adds the catalog after its own, `.catalog(&APP).catalog(&auth::yew023::L10N)`,
and translates watchword into Swedish by adding `watchword-*` messages to its
own `sv-SE.ftl` (the app's catalog wins a duplicate id) until watchword ships
`sv-SE` itself, checking them with `check::parity_with`. Without any provider, or with a provider that never added the
catalog, watchword renders its built-in English, as it does today. That change
is watchword's, on watchword's board.

## 3. Wiring up the apps

Each migration is its app's own card on its own board, after phrasebook is
published (or path-depended on locally). The same steps apply to all three:
add `phrasebook = { version = "0.1", features = ["yew023", "browser"] }`,
remove `fluent`, `unic-langid` and `fluent-syntax`, turn `l10n.rs` into a
catalog and a phrasebook constructor, and keep the `.ftl` files where they
are. The new code below names the handle variable `phrases`; an app may keep
calling it `l10n` at its existing call sites, which is only a matter of
naming.

### 3.1 army

```rust
// crates/web/src/l10n.rs, whole file
use phrasebook::{Catalog, Lang, Phrasebook, Source};
pub(crate) use phrasebook::{Phrases, args};

static L10N: Catalog = Catalog::new("army", &[
    Source::new(Lang::EN_US, include_str!("../locales/en-US.ftl")),
]);

pub(crate) fn phrasebook() -> Phrasebook {
    Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .catalog(&L10N)
        .catalog(&auth::yew023::L10N)
        .build()
}
```

- `Root` builds the handle once, replacing its own `use_memo(L10n::new)`:
  `use_memo((), |_| browser::initial(&l10n::phrasebook()))` (a
  `use_state` instead once army has a language to switch to), passes its
  messages to watchword's `AuthProvider`, and renders
  `ContextProvider<Phrases>` around it. `App` reads the provided handle with
  `use_context::<Phrases>()` instead of building a second `L10n` and its own
  `ContextProvider<L10n>`.
- `l10n.get(` becomes `phrases.text(` (230 sites) and `l10n.args(key, [...])`
  becomes `phrases.msg(key, &args([...]))` (74 sites), mixed-type ones
  `&args!(pts = number(points), limit = limit)`, which also stops
  stringifying counts.
- `armies::l10n(ctx)` becomes `phrasebook::yew023::or_default` over
  `ctx.link().context::<Phrases>(..)` mapped to its first element (2.10).
- `signed_out_notice` can go once watchword's own message says the same.
- **Deletes**: the `L10n` type, `format`, `bundle` and the tests in `l10n.rs`
  (about 105 of its 121 lines), the re-parse-per-call fallback, and three
  dependencies. Its logging is phrasebook's now. Its test becomes
  `check::catalog(&L10N)`.
- **Gains**: a language list ready for a second language, remembered choice,
  selector numbers.

### 3.2 mafia

`api::Locale` stays: it is the wire and database key for translated game
content. The frontend maps between it and `Lang`:

```rust
// crates/frontend/src/l10n.rs
pub(crate) use api::Locale;
pub(crate) use phrasebook::{Phrases, args};
use phrasebook::{Catalog, Lang, Phrasebook, Source};

const SV_SE: Lang = Lang::new("sv-SE");

static L10N: Catalog = Catalog::new("mafia", &[
    Source::new(Lang::EN_US, include_str!("../locales/en-US.ftl")),
    Source::new(SV_SE, include_str!("../locales/sv-SE.ftl")),
]);

pub(crate) fn phrasebook() -> Phrasebook {
    let mut builder = Phrasebook::builder();
    for locale in Locale::ALL {
        builder = builder.language(lang(locale), locale.label());
    }
    builder.catalog(&L10N).build()
}

pub(crate) fn lang(locale: Locale) -> Lang { Lang::parse(locale.code()).expect("a shipped locale") }
pub(crate) fn locale(lang: Lang) -> Locale { Locale::from_code(&lang.to_string()).unwrap_or(Locale::DEFAULT) }

/// `phrases.locale()` for the components picking server-side translations.
pub(crate) trait LocaleExt { fn locale(&self) -> Locale; }
impl LocaleExt for Phrases { fn locale(&self) -> Locale { locale(self.lang()) } }
```

- `App` keeps owning the handle, since it needs it above the provider for
  the tab title: `L10n::new(l10n::load())` becomes
  `browser::initial(&l10n::phrasebook())`; `Msg::SetLang` does
  `self.phrases = self.phrases.with_lang(lang); self.phrases.store();`
  (replacing `l10n::store`); `Msg::CycleLang` does `with_lang` without
  storing. It renders `ContextProvider<Phrases>`. The toolbar's
  `lang`/`onsetlang` props stay as they are: the picker's callback comes
  from `App`.
- The ~25 `strings.get(api::Field::..., l10n.lang())` become
  `phrases.locale()` (a rename at those sites, with `LocaleExt` in scope).
- The `.expect("Expected l10n context")` sites become
  `or_default(use_context::<Phrases>())` and, in struct components,
  `or_default` over `ctx.link().context` mapped to its first element (2.10),
  which do not panic.
- **Deletes**: `langid`, `detect`, `load`, `store`, `STORAGE_KEY`, `Inner`,
  `bundle`, `format_in`, the `L10n` type and `args` (about 180 of 261 lines;
  what stays is the catalog and the mapping above), the hand-matched
  per-language fields, and `fluent` and `unic-langid`. Tests: every-key and
  parity become `check::catalog(&L10N)` and `check::parity(&L10N, SV_SE)`;
  the detection test moves into phrasebook.
- **Gains**: logging of missing Swedish messages, `navigator.languages`
  detection, and adding a language is a `Locale` variant and a `.ftl` file.

### 3.3 livemix

`api::Locale` stays for its flags, labels and `from_iso`; `Locale::ALL` still
drives both the picker and the builder.

```rust
// crates/web/src/l10n.rs
pub(crate) use api::Locale;
pub(crate) use phrasebook::{Args, Phrases, Value, args};

static L10N: Catalog = Catalog::new("livemix", &[
    Source::new(Lang::EN_US, include_str!("../locales/en-US.ftl")),
]);

pub(crate) fn phrasebook() -> Phrasebook { /* as mafia, from Locale::ALL */ }
pub(crate) fn lang(locale: Locale) -> Lang { Lang::parse(&locale.code()).expect("a shipped locale") }
```

- `App` keeps owning the handle: its `L10n::new(crate::l10n::load())` and
  `Msg::SetLang` change as in mafia; `LanguageSelector` keeps emitting
  `Msg::SetLang` through its callback and maps the chosen `Locale` with
  `lang()`.
- `ctx.rs`'s `impl ContextFallback for L10n` becomes `impl ContextFallback
  for Phrases` (a local trait on a foreign type is allowed), with
  `fallback()` returning `phrasebook().phrases(Lang::EN_US)`; its
  `use_ctx::<L10n>()` sites name `Phrases` instead, and the `link_ctx` sites
  do not change. Alternatively livemix drops that impl and uses
  `or_default(use_context::<Phrases>())`, whose fallback renders ids rather
  than English.
- `text` returns `Rc<str>` instead of `AttrValue`: props and children accept
  it unchanged; the few places that name `AttrValue` as the binding's type
  add `.into()`.
- `FluentArgs`/`FluentValue` in signatures become `Args`/`Value`;
  `FluentValue::from(count)` becomes `Value::from(count)` or plain `count`.
- `sweep` finds asks by grepping for `l10n.text("` and `l10n.msg("`: the
  method names are the same, so it keeps matching if livemix keeps naming
  the variable `l10n`, and its two patterns become `phrases.text("` and
  `phrases.msg("` if the call sites rename it.
- **Deletes**: `args`, `langid`, `detect`, `load`, `store`, `STORAGE_KEY`,
  `Inner`, `bundle`, `format_in`, `selectors`, `expression_selectors`,
  `numbered`, the `L10n` type and its cache (lines 1 to 260), and `fluent`,
  `fluent-syntax` and `unic-langid`. Tests: the catalog-parses and
  every-key-resolves tests become `check::catalog`; the selector-number,
  unselected-digits, placeable and detection tests move into phrasebook's own
  suite, where they cover every app. Kept in livemix: the vocabulary tests,
  `every_key_the_wire_contract_names_resolves`, the clock vocabulary, the
  `-detail` rule and the flag test, all rewritten onto
  `phrasebook().phrases(..)`.
- **Gains**: a second language is a `Locale` constant in `ALL` and a `.ftl`;
  missing messages are logged.

## 4. Decisions

1. **One dictionary, many cheap handles.** The bundles live behind one `Rc`
   in `Phrasebook`; a `Phrases` is that `Rc` plus a `Lang`, compared by pointer
   and language, as in all three apps. Switching language is `with_lang`, so
   a re-render happens exactly when the language changes.
2. **phrasebook's own `Lang`, apps keep their `Locale`.** Each app's locale
   type carries app concerns (mafia's database column and wire format,
   livemix's flags). A generic `Phrases<L>` would let apps use theirs directly,
   but a library component cannot name the app's type, so the context type
   has to be concrete. Apps convert with a one-line function each way.
3. **Fluent stays out of the API.** fluent and unic-langid are pre-1.0;
   exposing `FluentArgs` would mean a `fluent017` feature and a breaking
   release per fluent upgrade. `Args`/`Value` cost one wrapper each.
4. **Catalogs in builder order, first wins.** Library ids are prefixed, so
   collisions are mistakes, except where an app deliberately overrides or
   translates a library message, which is why the app's catalog goes first
   and wins. A duplicate is logged at build and fails `check::catalog`.
5. **The context type is `Phrases`, provided and read with yew's own
   tools.** Every app's root owns the handle as app state and renders
   `ContextProvider<Phrases>`; components read it with `use_context`. A
   provider component and hooks would only wrap those, and a callback inside
   the handle would duplicate what the app already has: changing language is
   `with_lang` (and `store`) at the root, and a deep component (a picker)
   gets a callback from the app like for any other app state.
6. **Bidi isolation off by default.** All three apps turn it off: in HTML
   text nodes the isolation marks show as stray characters. A right-to-left
   app can turn it on with `use_isolating(true)`.
7. **A missing message renders its id and is logged once.** Visible and
   searchable; the log names the language, which mafia's silent fallback to
   English never did.
8. **No provider is not a panic.** A component outside a provider (a test, a
   portal, a library used without phrasebook set up) renders ids, or a
   library's built-in English, with one warning, as livemix does. These are
   plain functions over what `use_context` returns, `or_default` and
   `or_catalog` (2.10), rather than hooks.
9. **Bundles parse lazily, per language.** livemix's en-US is 3,256 lines;
   parsing languages nobody is reading only slows start-up.

### Decisions by the user (2026-10-15, card ea9973ec)

1. The design is approved.
2. koine is the optional `koine01` feature; the core checks only the tag's
   shape.
3. **One crate**, rather than a second phrasebook-yew023 crate: its
   `yew023` code needs no yew macros because it has no components (see 2.1).
4. Digit strings passed to a selector variable are read as numbers, as
   livemix does.
5. (2026-10-15, via the foreman) **No components or hooks.** `yew023` is
   trait impls on `Phrases` (and `Lang`) plus the plain fallback helpers
   `or_default` and `or_catalog`; apps provide and read the handle with
   yew's own `ContextProvider` and `use_context` (2.10).
6. (2026-10-15, via the foreman) The handle is named **`Phrases`**: the
   phrasebook open at the current language. `Phrasebook::l10n` is
   `Phrasebook::phrases`.
