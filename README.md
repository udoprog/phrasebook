# phrasebook

[<img alt="github" src="https://img.shields.io/badge/github-udoprog/phrasebook-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/phrasebook)
[<img alt="crates.io" src="https://img.shields.io/crates/v/phrasebook.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/phrasebook)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-phrasebook-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/phrasebook)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/udoprog/phrasebook/ci.yml?branch=main&style=for-the-badge" height="20">](https://github.com/udoprog/phrasebook/actions?query=branch%3Amain)

Fluent localization for Rust web frontends: message catalogs per
language, formatting with arguments and plurals, a fallback language, and
the user's chosen language remembered in the browser.

An app declares its languages and catalogs once, in a [`Phrasebook`], and
hands its components a [`Phrases`] handle: the phrasebook open at the
current language. Components look messages up with [`Phrases::text`] and
[`Phrases::msg`]. A library ships its own [`Catalog`] under a prefix,
which an app adds to its phrasebook and can override or translate, and
which the library falls back to when the app provides nothing.

The design is in [`docs/design.md`].

<br>

## Features

No feature is on by default. On docs.rs, items that need a feature are
marked with it.

| Feature | Module | What it provides |
|---|---|---|
| *(none)* | | [`Lang`], [`Catalog`], [`Source`], [`Phrasebook`], [`Phrases`], [`Args`], [`Value`], [`args!`], and the [`check`] test helpers |
| `browser` | [`browser`] | the language remembered in localStorage, the browser's preferred languages, and `Phrases::store` |
| `yew023` | [`yew023`] | yew 0.23: `ImplicitClone` and `IntoPropValue` impls, and the [`or_default`] and [`or_catalog`] helpers for components rendered without a provided handle |
| `koine01` | [`koine01`] | conversions between [`Lang`] and koine 0.1's `Language`, `Country` and `Locale`, a warning for declared languages koine does not know, and `Builder::language_named_by_koine` |

Fluent and unic-langid are used underneath but appear nowhere in the
API, so upgrading them is never a breaking change. A dependency that is
not yet 1.0 and does appear in the API is named after its version in its
feature and module, so a later version can be added beside it.

`yew023` does not enable `browser`; an app in the browser enables both.

<br>

## Example

```rust
use phrasebook::{Catalog, Lang, Phrasebook, Source, args};

const SV_SE: Lang = Lang::new("sv-SE");

static APP: Catalog = Catalog::new("app", &[
    Source::new(Lang::EN_US, "
title = Mafia
players = { $count ->
    [one] One player
   *[other] { $count } players
}
"),
    Source::new(SV_SE, "
title = Maffia
"),
]);

// The first declared language is the fallback.
let book = Phrasebook::builder()
    .language(Lang::EN_US, "English")
    .language(SV_SE, "Svenska")
    .catalog(&APP)
    .build();

let phrases = book.phrases(SV_SE);

assert_eq!(&*phrases.text("title"), "Maffia");
// Not translated yet: the fallback language, logged once.
assert_eq!(phrases.msg("players", &args!(count = 1)), "One player");
// Not defined anywhere: the id, so the hole is visible.
assert_eq!(&*phrases.text("missing"), "missing");

let phrases = phrases.with_lang(Lang::EN_US);
assert_eq!(phrases.msg("players", &args!(count = "4")), "4 players");
```

In an app's tests, [`check`] turns a catalog's problems (syntax errors,
duplicate ids, ids outside the prefix, messages that do not format, and
untranslated or extra messages) into failures. `check::all` runs every
check of a catalog in every language at once, naming the library catalogs
the app translates, and `Phrasebook::has_catalog` asserts that a
library's catalog was added. `check::coverage` compares the ids the code
asks for with the ones a catalog defines, both ways.

<br>

## With yew

phrasebook ships no components and no hooks: the context type is
[`Phrases`] itself. The app's root owns the handle as app state and
provides it with yew's own `ContextProvider<Phrases>`; on a language change
it provides `phrases.with_lang(lang)` (and, with `browser`, calls
`browser::set_document_lang` and `store`). Components read it with `use_context::<Phrases>()`, or
`ctx.link().context::<Phrases>(..)` in a struct component.

```rust
// The app's root, in `view`.
html! {
    <ContextProvider<Phrases> context={self.phrases.clone()}>
        <Main />
    </ContextProvider<Phrases>>
}

// An app's component.
let phrases = or_default(use_context::<Phrases>());

// A library's component, with its own catalog `L10N`.
let phrases = or_catalog(use_context::<Phrases>(), &L10N);
```

A library's component falls back to its own catalog for messages the app
lacks, or to the catalog alone when nothing is provided. The [`yew023`]
module documents both forms in full, struct components included.

To put markup where a translation places an argument, such as a user's
name in `<strong>`, `yew023::html_around` renders the message around an
element (`Phrases::msg_around` gives the text on either side).

<br>

## Examples

[`examples/tour.rs`] prints what an app's catalog, a library's catalog
with a backstop, plurals and the fallback language produce, in English
and Swedish:

```sh
cargo run --example tour
```

[`examples/app`] is a small Yew app: the root owns and provides the
handle, a picker switches and remembers the language, and a library-style
component falls back to its own catalog. It is its own cargo workspace,
so it adds nothing to the crate's builds, and is built and served with
[trunk]:

```sh
cd examples/app && trunk serve
```

[`Args`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Args.html
[`args!`]: https://docs.rs/phrasebook/latest/phrasebook/macro.args.html
[`browser`]: https://docs.rs/phrasebook/latest/phrasebook/browser/index.html
[`Catalog`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Catalog.html
[`check`]: https://docs.rs/phrasebook/latest/phrasebook/check/index.html
[`docs/design.md`]: https://github.com/udoprog/phrasebook/blob/main/docs/design.md
[`examples/app`]: https://github.com/udoprog/phrasebook/tree/main/examples/app
[`examples/tour.rs`]: https://github.com/udoprog/phrasebook/blob/main/examples/tour.rs
[`koine01`]: https://docs.rs/phrasebook/latest/phrasebook/koine01/index.html
[`Lang`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Lang.html
[`or_catalog`]: https://docs.rs/phrasebook/latest/phrasebook/yew023/fn.or_catalog.html
[`or_default`]: https://docs.rs/phrasebook/latest/phrasebook/yew023/fn.or_default.html
[`Phrasebook`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Phrasebook.html
[`Phrases::msg`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Phrases.html#method.msg
[`Phrases::text`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Phrases.html#method.text
[`Phrases`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Phrases.html
[`Source`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Source.html
[`Value`]: https://docs.rs/phrasebook/latest/phrasebook/struct.Value.html
[`yew023`]: https://docs.rs/phrasebook/latest/phrasebook/yew023/index.html
[trunk]: https://trunkrs.dev
