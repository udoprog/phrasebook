//! [<img alt="github" src="https://img.shields.io/badge/github-udoprog/phrasebook-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/phrasebook)
//! [<img alt="crates.io" src="https://img.shields.io/crates/v/phrasebook-catalog.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/phrasebook-catalog)
//! [<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-phrasebook--catalog-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/phrasebook-catalog)
//!
//! The language identifier and message catalog types of [phrasebook]:
//! [`Lang`], [`Source`] and [`Catalog`]. They are plain `const` data, with
//! no dependencies by default and no need for `std` or `alloc`.
//!
//! Use them through phrasebook, which re-exports them at its crate root as
//! `phrasebook::{Lang, Source, Catalog, ParseLangError}`. They live in this
//! crate of their own so that a library's catalog outlives phrasebook's
//! breaking releases: a library such as watchword ships its messages as a
//! `pub static L10N: Catalog`, and an app hands that catalog to its own
//! phrasebook. Both name this crate's `Catalog`, which changes far more
//! rarely than phrasebook does, so an app can move to a new phrasebook
//! without waiting for every library it uses to move with it.
//!
//! <br>
//!
//! ## Features
//!
//! None is on by default. Each versioned feature adds trait impls on
//! [`Lang`] for a dependency that is not yet 1.0, which phrasebook's feature
//! of the same name turns on; they live here because only the crate that
//! defines `Lang` may implement another crate's traits for it. A later
//! version of such a dependency is another feature beside it, not a breaking
//! release.
//!
//! | Feature | What it provides |
//! |---|---|
//! | *(none)* | [`Lang`], [`ParseLangError`], [`Source`] and [`Catalog`] |
//! | `koine03` | [`koine03`]: `TryFrom` between [`Lang`] and koine 0.3's `Locale` and `Language`, the conversion functions they are built on, and their `Error` |
//! | `yew023` | yew 0.23's `ImplicitClone` for [`Lang`], and `IntoPropValue` from [`Lang`] into `AttrValue` and `Option<AttrValue>`, for an element's `lang` attribute |
//!
//! <br>
//!
//! ## Example
//!
//! ```
//! use phrasebook_catalog::{Catalog, Lang, Source};
//!
//! const SV_SE: Lang = Lang::new("sv-SE");
//!
//! pub static L10N: Catalog = Catalog::new("watchword", &[
//!     Source::new(Lang::EN_US, "watchword-sign-in = Sign in"),
//!     Source::new(SV_SE, "watchword-sign-in = Logga in"),
//! ])
//! .with_prefix("watchword-");
//!
//! assert_eq!(L10N.default_lang(), Some(Lang::EN_US));
//! assert_eq!(L10N.source(SV_SE).map(|s| s.ftl()), Some("watchword-sign-in = Logga in"));
//! ```
//!
//! [`Catalog`]: https://docs.rs/phrasebook-catalog/latest/phrasebook_catalog/struct.Catalog.html
//! [`koine03`]: https://docs.rs/phrasebook-catalog/latest/phrasebook_catalog/koine03/index.html
//! [`Lang`]: https://docs.rs/phrasebook-catalog/latest/phrasebook_catalog/struct.Lang.html
//! [`ParseLangError`]: https://docs.rs/phrasebook-catalog/latest/phrasebook_catalog/struct.ParseLangError.html
//! [`Source`]: https://docs.rs/phrasebook-catalog/latest/phrasebook_catalog/struct.Source.html
//! [phrasebook]: https://docs.rs/phrasebook

#![no_std]
#![cfg_attr(phrasebook_docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

#[cfg(feature = "yew023")]
extern crate alloc;

#[cfg(test)]
extern crate std;

mod catalog;
mod lang;

#[cfg(feature = "koine03")]
#[cfg_attr(phrasebook_docsrs, doc(cfg(feature = "koine03")))]
pub mod koine03;

#[cfg(feature = "yew023")]
mod yew023;

pub use self::catalog::{Catalog, Source};
pub use self::lang::{Lang, ParseLangError};
