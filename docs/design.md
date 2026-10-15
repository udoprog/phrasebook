# phrasebook design

phrasebook is the Fluent localization layer of a Rust web frontend: one
Fluent bundle per supported language, built from `.ftl` resources compiled
into the binary; message lookup with arguments and plurals; a fallback
language and visible missing-message behaviour; the user's chosen language
remembered in the browser; and a Yew context, provider and hooks (behind a
versioned `yew023` feature) through which any component looks messages up,
including components of a library such as watchword, which ship their own
messages and fall back to built-in English when the app provides nothing.

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

Once implemented, the crate docs (`src/lib.rs` and the items' own docs) are
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

Two crates, released in lockstep and pinning each other with `=` versions, as
watchword does:

- **phrasebook**: the language identifier, catalogs, the dictionary, the
  `L10n` handle, arguments, and the browser helpers. No yew. It re-exports
  phrasebook-yew023 as `phrasebook::yew023` behind the `yew023` feature, so an
  app needs only `phrasebook = { features = ["yew023"] }`.
- **phrasebook-yew023**: the provider component, hooks and struct-component
  helpers for yew 0.23. It depends on yew under the key `yew` because yew's
  macros (`html!`, `#[function_component]`, `#[derive(Properties)]`,
  `#[hook]`) expand to `::yew::` paths, the reason watchword has
  watchword-yew023. A later yew is another crate beside it.

No feature is on by default.

| Feature | Enables | Pulls in |
|---|---|---|
| *(none)* | `Lang`, `Catalog`, `Source`, `Phrasebook`, `L10n`, `Args`, `Value`, `args!`, the `check` test helpers | fluent-bundle, fluent-syntax, unic-langid, tracing |
| `browser` | `phrasebook::browser`: remembered language in localStorage, browser preferences | web-sys, wasm-bindgen |
| `koine01` | `phrasebook::koine01`: conversions between `Lang` and koine 0.1's language and country records | koine 0.1 (`language`, `country`, `locale` features) |
| `yew023` | `phrasebook::yew023` (phrasebook-yew023), implies `browser` | yew 0.23 |

**Versioned dependencies.** Per the user's convention, a pre-1.0 dependency
that appears in the public API is named after its version. fluent (0.17,
fluent-bundle 0.16) and unic-langid (0.9) are not 1.0 and are kept out of the
API entirely: `Args`, `Value` and `Lang` are phrasebook's own types, so a
fluent upgrade is an internal change rather than a `fluent017` feature. yew is
in the API (the hooks), hence `yew023`. koine is not 1.0 and its records appear
in the API, hence `koine01` (see 2.8). web-sys appears nowhere in the API, so
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
store (`en-US`). It is phrasebook's own type and not koine's because koine
provides the ISO tables and lookups (`koine::language::by_part1`, `by_id`,
`koine::country::by_alpha2`, `koine::locale::contains`) as `&'static` records,
not a compact identifier; the compact `Language`/`Country` newtypes livemix and
track use live in their `api` crates. Validating a tag against ISO 639 means
linking koine's language table (about 106k generated lines in the current
copies), which every app would carry in its wasm for a check its own
declaration already makes; so the core checks only the shape and koine is
behind `koine01` (see 2.8 and question 2).

### 2.3 Catalogs: an app's or a library's messages

```rust
/// One language's `.ftl` source, embedded in the binary.
pub struct Source { /* lang, ftl */ }
impl Source { pub const fn new(lang: Lang, ftl: &'static str) -> Source; }

/// Everything one crate ships: its name, the prefix every message id it
/// defines must start with (if any), and one source per language.
pub struct Catalog { /* name, prefix, sources */ }
impl Catalog {
    pub const fn new(name: &'static str, sources: &'static [Source]) -> Catalog;
    pub const fn with_prefix(self, prefix: &'static str) -> Catalog;
    pub fn languages(&self) -> impl Iterator<Item = Lang>;
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
falls back to when used without a provider (2.7). Sources stay `&'static str`
and are parsed only when a bundle for that language is first needed.

### 2.4 `Phrasebook`: the dictionary

```rust
#[derive(Clone)]
pub struct Phrasebook { /* Rc<Inner> */ }

impl Phrasebook {
    pub fn builder() -> Builder;
    /// A handle in `lang`, or in the fallback if `lang` is not declared.
    pub fn l10n(&self, lang: Lang) -> L10n;
    pub fn languages(&self) -> &[Language];
    pub fn fallback(&self) -> Lang;
    /// The best declared language for a list of preferred tags, most
    /// preferred first: an exact match, then the first declared language
    /// with the same primary subtag, then the fallback.
    pub fn negotiate<'a>(&self, preferred: impl IntoIterator<Item = &'a str>) -> Lang;
}

/// A supported language and its name in itself, for a language picker.
pub struct Language { pub lang: Lang, pub name: &'static str }

impl Builder {
    /// Declare a supported language, in picker order. The first is the
    /// fallback unless `fallback` says otherwise.
    pub fn language(self, lang: Lang, name: &'static str) -> Self;
    pub fn fallback(self, lang: Lang) -> Self;
    /// Add a catalog. Earlier catalogs win a duplicate id, so the app adds
    /// its own first and can override a library's message (see 4.4).
    pub fn catalog(self, catalog: &'static Catalog) -> Self;
    /// Bidi isolation marks around placeables; off by default (4.6).
    pub fn use_isolating(self, on: bool) -> Self;
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

### 2.5 `L10n`: the handle components use

```rust
#[derive(Clone)]
pub struct L10n { /* Phrasebook, Lang, Option<switch> */ }
// PartialEq: same Phrasebook (pointer) and same language.

impl L10n {
    pub fn lang(&self) -> Lang;
    pub fn languages(&self) -> &[Language];
    pub fn with_lang(&self, lang: Lang) -> L10n;

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

    /// Ask whoever provides this handle to switch language and remember the
    /// choice (2.6, 2.7). Without a provider: a warning, and nothing happens.
    pub fn set_lang(&self, lang: Lang);
    /// Switch without remembering, as a projector page cycling through
    /// languages does.
    pub fn show_lang(&self, lang: Lang);
    /// Attach what `set_lang`/`show_lang` call; used by providers.
    pub fn on_switch(self, switch: impl Fn(Lang, Remember) + 'static) -> L10n;
}

/// Whether a switch is remembered (`set_lang`) or only shown (`show_lang`).
pub enum Remember { Yes, No }
```

`text` and `msg` keep the names livemix and mafia use, so their call sites and
livemix's `sweep` (which greps for `l10n.text("` and `l10n.msg("`) carry over
unchanged; army's `get` and `args` are renamed (section 3).

**Lookup.** A message is formatted in the first of these that defines it:
the handle's language; the fallback language; the backstop catalog of a
library hook (2.7); and otherwise the result is the id itself, so the hole is
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
    pub fn remember(lang: Lang, key: &str);
    /// `navigator.languages`, most preferred first.
    pub fn preferred() -> Vec<String>;
    /// The language to start in: the remembered choice, else the browser's
    /// preferences negotiated against the declared languages, else the
    /// fallback.
    pub fn initial(book: &Phrasebook, key: &str) -> Lang;
}
```

The key and value format (`"lang"`, `"en-US"`) are the apps' current ones, so
users keep their choice across the switch. Detection reads the whole
`navigator.languages` list rather than `navigator.language`, so a user
preferring `de, sv` gets Swedish from mafia instead of English. Outside wasm
(an app's host-side tests and clippy) these return nothing and store nothing.

### 2.8 koine (`koine01`)

koine (`~/repo/koine`) is being extracted on its own board (card 9f83ed50) and
has no code yet. This section designs against what that card commits to: a
crate `koine` with `koine::language` (ISO 639), `koine::country` (ISO 3166-1)
and `koine::locale` (valid language and country pairs), each behind a feature,
with the lookups the current `iso639`, `iso3166` and `locales` crates have:
`language::by_part1(&str)`, `language::by_id(&str)` returning a record with
`id`, `part1`, `name` (the English reference name) and `flag`;
`country::by_alpha2(&str)` returning a record with `alpha2`, `name` and
`has_flag`; and `locale::contains(language_id, alpha2)`. Names here follow
those crates and get adjusted to whatever koine publishes.

```rust
pub mod koine01 {
    pub fn language(lang: Lang) -> Option<&'static koine::language::Language>;
    pub fn country(lang: Lang) -> Option<&'static koine::country::Country>;
    /// The language's part 1 code (or id, where it has none) and the country.
    pub fn lang(language: &koine::language::Language, country: Option<&koine::country::Country>) -> Option<Lang>;
    /// Whether koine knows the pair as a valid locale.
    pub fn is_known(lang: Lang) -> bool;
}
```

With `koine01` on, `Builder::build` also warns about a declared language koine
does not know, and `Language::name` can be left to koine's reference name
(`Builder::language_named_by_koine(lang)`) when an app has no endonym to hand.
If koine gains a compact Copy identifier, `Lang` converts to and from it here.

### 2.9 Checks for an app's or library's tests

```rust
pub mod check {
    /// Every source parses; no id is defined twice; every id has the
    /// catalog's prefix; every message formats without errors given
    /// placeholder arguments.
    pub fn catalog(catalog: &Catalog) -> Result<(), Problems>;
    /// `lang` defines exactly the ids of the catalog's default language.
    pub fn parity(catalog: &Catalog, lang: Lang) -> Result<(), Problems>;
    /// The message ids a catalog defines in one language, in file order.
    pub fn ids(catalog: &Catalog, lang: Lang) -> Vec<String>;
}
```

These replace each app's "the catalog parses, every key resolves" tests and
mafia's sv-SE parity test. livemix's vocabulary rules stay in livemix, built
on `ids` and an `L10n`.

### 2.10 Yew (`yew023`)

The context type is `L10n` itself, so a component looks messages up the same
way whatever provides it.

```rust
pub mod yew023 {
    /// Owns the current language: starts in `browser::initial`, and gives
    /// its children an `L10n` whose `set_lang` switches and remembers and
    /// whose `show_lang` switches only.
    #[function_component]
    pub fn L10nProvider(props: &L10nProviderProps) -> Html;

    #[derive(Properties, PartialEq)]
    pub struct L10nProviderProps {
        pub phrasebook: Phrasebook,
        #[prop_or(AttrValue::Static(browser::DEFAULT_KEY))]
        pub storage_key: AttrValue,
        /// Told about every switch, for an app that also keeps the language
        /// (to update the tab title, say).
        #[prop_or_default]
        pub onchange: Callback<Lang>,
        pub children: Html,
    }

    /// The provided `L10n`. Without a provider: a shared empty one that
    /// renders ids, built once per thread, and one warning.
    #[hook]
    pub fn use_l10n() -> L10n;

    /// For a library component: the provided `L10n` backed by the library's
    /// catalog, so a message the app's bundles lack (the app did not add the
    /// catalog, or not in this language) comes from the library's default
    /// language. Without a provider: the library's catalog alone, in its
    /// default language, built once per thread.
    #[hook]
    pub fn use_l10n_or(catalog: &'static Catalog) -> L10n;

    /// For struct components: the `L10n` and the subscription handle to keep,
    /// `None` on the fallback path. `callback` receives later changes.
    pub fn context<C: Component>(link: &Scope<C>, callback: Callback<L10n>)
        -> (L10n, Option<ContextHandle<L10n>>);
    pub fn context_or<C: Component>(link: &Scope<C>, callback: Callback<L10n>, catalog: &'static Catalog)
        -> (L10n, Option<ContextHandle<L10n>>);
}
```

An app that keeps the language in its own root component (mafia, livemix)
need not use `L10nProvider`: it builds `phrasebook.l10n(lang).on_switch(...)`
itself and renders yew's `ContextProvider<L10n>`, and every hook above works
the same, because the context type is the same.

phrasebook ships no language picker: the apps' pickers differ in looks (flags,
a context menu, a toolbar select) and need only `l10n.languages()`,
`l10n.lang()` and `l10n.set_lang()`.

### 2.11 Library components (watchword)

watchword-yew023 has about a hundred English strings inline (`"Sign in"`,
`"Delete {name}"`, ...) and takes a few as props (`AuthProvider`'s `title` and
`signed_out_notice`, default `SESSION_ENDED`). With phrasebook it:

1. adds `crates/watchword-yew023/locales/en-US.ftl` with every string under
   the `watchword-` prefix (`watchword-sign-in`, `watchword-delete = Delete {
   $name }`), and exports `pub static L10N: phrasebook::Catalog` from
   `watchword::yew023`;
2. calls `use_l10n_or(&L10N)` (or `context_or`) in each component and looks
   its strings up by id;
3. keeps the props as overrides, defaulting to its own messages, so an app
   passing them today is unaffected;
4. tests its catalog with `check::catalog(&L10N)`, which also enforces the
   prefix.

An app adds the catalog after its own, `.catalog(&APP).catalog(&auth::yew023::L10N)`,
and translates watchword into Swedish by adding `watchword-*` messages to its
own `sv-SE.ftl` (the app's catalog wins a duplicate id) until watchword ships
`sv-SE` itself. Without any provider, or with a provider that never added the
catalog, watchword renders its built-in English, as it does today. That change
is watchword's, on watchword's board.

## 3. Wiring up the apps

Each migration is its app's own card on its own board, after phrasebook is
published (or path-depended on locally). The same steps apply to all three:
add `phrasebook = { version = "0.1", features = ["yew023"] }`, remove
`fluent`, `unic-langid` and `fluent-syntax`, turn `l10n.rs` into a catalog and
a phrasebook constructor, and keep the `.ftl` files where they are.

### 3.1 army

```rust
// crates/web/src/l10n.rs, whole file
use phrasebook::{Catalog, Lang, Phrasebook, Source};
pub(crate) use phrasebook::{L10n, args};

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

- `main` renders `<L10nProvider phrasebook={l10n::phrasebook()}><Root /></L10nProvider>`;
  `Root` uses `use_l10n()` instead of its own `use_memo(L10n::new)`, and `App`
  takes the provided handle instead of building a second `L10n` and its own
  `ContextProvider<L10n>`.
- `l10n.get(` becomes `l10n.text(` (230 sites) and `l10n.args(key, [...])`
  becomes `l10n.msg(key, &args([...]))` (74 sites), mixed-type ones
  `&args!(pts = number(points), limit = limit)`, which also stops
  stringifying counts.
- `armies::l10n(ctx)` becomes `phrasebook::yew023::context(ctx.link(), ...)`.
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
pub(crate) use phrasebook::{L10n, args};
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

/// `l10n.locale()` for the components picking server-side translations.
pub(crate) trait LocaleExt { fn locale(&self) -> Locale; }
impl LocaleExt for L10n { fn locale(&self) -> Locale { locale(self.lang()) } }
```

- `App` keeps owning the language, since it needs it above the provider for
  the tab title: `L10n::new(l10n::load())` becomes
  `phrasebook().l10n(browser::initial(&book, DEFAULT_KEY)).on_switch(link callback)`;
  `Msg::SetLang` keeps `with_lang`, and `l10n::store` becomes
  `browser::remember`; `Msg::CycleLang` keeps `with_lang` without
  remembering. It still renders `ContextProvider<L10n>`. The toolbar's
  `lang`/`onsetlang` props can stay as they are or switch to
  `l10n.set_lang`.
- The ~25 `strings.get(api::Field::..., l10n.lang())` become
  `l10n.locale()` (a rename at those sites, with `LocaleExt` in scope).
- The `.expect("Expected l10n context")` sites become `use_l10n()` and
  `phrasebook::yew023::context(..)`, which do not panic.
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
pub(crate) use phrasebook::{Args, L10n, Value, args};

static L10N: Catalog = Catalog::new("livemix", &[
    Source::new(Lang::EN_US, include_str!("../locales/en-US.ftl")),
]);

pub(crate) fn phrasebook() -> Phrasebook { /* as mafia, from Locale::ALL */ }
pub(crate) fn lang(locale: Locale) -> Lang { Lang::parse(&locale.code()).expect("a shipped locale") }
```

- `App`'s `L10n::new(crate::l10n::load())` and `Msg::SetLang` change as in
  mafia; `LanguageSelector` maps the chosen `Locale` with `lang()`.
- `ctx.rs`'s `impl ContextFallback for L10n` stays (a local trait on a foreign
  type is allowed), with `fallback()` returning `phrasebook().l10n(Lang::EN_US)`;
  its `use_ctx::<L10n>()` and `link_ctx` sites do not change. Alternatively livemix
  drops that impl and uses `phrasebook::yew023::use_l10n`, which does the
  same.
- `text` returns `Rc<str>` instead of `AttrValue`: props and children accept
  it unchanged; the few places that name `AttrValue` as the binding's type
  add `.into()`.
- `FluentArgs`/`FluentValue` in signatures become `Args`/`Value`;
  `FluentValue::from(count)` becomes `Value::from(count)` or plain `count`.
- `sweep` is unchanged: the call names are the same.
- **Deletes**: `args`, `langid`, `detect`, `load`, `store`, `STORAGE_KEY`,
  `Inner`, `bundle`, `format_in`, `selectors`, `expression_selectors`,
  `numbered`, the `L10n` type and its cache (lines 1 to 260), and `fluent`,
  `fluent-syntax` and `unic-langid`. Tests: the catalog-parses and
  every-key-resolves tests become `check::catalog`; the selector-number,
  unselected-digits, placeable and detection tests move into phrasebook's own
  suite, where they cover every app. Kept in livemix: the vocabulary tests,
  `every_key_the_wire_contract_names_resolves`, the clock vocabulary, the
  `-detail` rule and the flag test, all rewritten onto `phrasebook().l10n(..)`.
- **Gains**: a second language is a `Locale` constant in `ALL` and a `.ftl`;
  missing messages are logged.

## 4. Decisions

1. **One dictionary, many cheap handles.** The bundles live behind one `Rc`
   in `Phrasebook`; an `L10n` is that `Rc` plus a `Lang`, compared by pointer
   and language, as in all three apps. Switching language is `with_lang`, so
   a re-render happens exactly when the language changes.
2. **phrasebook's own `Lang`, apps keep their `Locale`.** Each app's locale
   type carries app concerns (mafia's database column and wire format,
   livemix's flags). A generic `L10n<L>` would let apps use theirs directly,
   but a library component cannot name the app's type, so the context type
   has to be concrete. Apps convert with a one-line function each way.
3. **Fluent stays out of the API.** fluent and unic-langid are pre-1.0;
   exposing `FluentArgs` would mean a `fluent017` feature and a breaking
   release per fluent upgrade. `Args`/`Value` cost one wrapper each.
4. **Catalogs in builder order, first wins.** Library ids are prefixed, so
   collisions are mistakes, except where an app deliberately overrides or
   translates a library message, which is why the app's catalog goes first
   and wins. A duplicate is logged at build and fails `check::catalog`.
5. **The context type is `L10n`, and providing it is open.** `L10nProvider`
   covers an app with no reason to own the language (army); an app that does
   own it (mafia's title, livemix's `App` state) provides `L10n` itself. The
   switch callback rides inside `L10n` so a deep component (a picker) can ask
   for a change without a second context.
6. **Bidi isolation off by default.** All three apps turn it off: in HTML
   text nodes the isolation marks show as stray characters. A right-to-left
   app can turn it on with `use_isolating(true)`.
7. **A missing message renders its id and is logged once.** Visible and
   searchable; the log names the language, which mafia's silent fallback to
   English never did.
8. **No provider is not a panic.** A component outside a provider (a test, a
   portal, a library used without phrasebook set up) renders ids, or a
   library's built-in English, with one warning, as livemix does.
9. **Bundles parse lazily, per language.** livemix's en-US is 3,256 lines;
   parsing languages nobody is reading only slows start-up.

### Open questions for the user

1. Approve this design so implementation (card 53f69b29) can start.
2. koine as the optional `koine01` feature (recommended: the core only needs
   the tag's shape, and validating against ISO 639 would link koine's large
   language table into every app's wasm), or a required dependency with
   `Lang` built on koine's records.
3. Two crates, phrasebook and phrasebook-yew023 (recommended, watchword's
   layout, forced by yew's macros), or one crate whose yew code avoids the
   macros (hand-written `Component` and `Properties` impls).
4. Keep livemix's reading of digit strings as numbers in selectors
   (recommended: it makes army's and livemix's stringified counts pick the
   right plural with no call-site change), or drop it and rely on `args!`
   passing real numbers.
