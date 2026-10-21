//! Integration with yew 0.23.
//!
//! The context type is [`Phrases`] itself: an app provides it with yew's
//! own `ContextProvider<Phrases>`, and components read it with
//! [`use_phrases`], or [`PhrasesContext`] in a struct component. This module
//! adds:
//!
//! * the hooks [`use_phrases`] and [`use_phrases_with`], and
//!   [`PhrasesContext`], the subscription a struct component keeps;
//! * [`ImplicitClone`] for [`Phrases`], [`Lang`] and [`Language`], so they
//!   pass as props and context like yew's own cheap values;
//! * [`IntoPropValue`] from [`Lang`] into `AttrValue`, for an element's
//!   `lang` attribute (the `Rc<str>` from [`Phrases::text`] already converts
//!   into `AttrValue` and into children);
//!
//! The impls on [`Lang`] are phrasebook-catalog's, which defines `Lang`;
//! this feature turns on its `yew023` feature.
//! * [`html_around`], for markup where a translation places an argument,
//!   such as a user's name in `<strong>`;
//! * [`or_default`] and [`or_catalog`], the plain functions those are built
//!   on, over the `Option<Phrases>` that `use_context` returns, for a
//!   component rendered where nothing provides a handle.
//!
//! # Providing the handle
//!
//! The app's root owns the handle as app state. On a language change it
//! calls [`Phrases::with_lang`] (and with the `browser` feature,
//! `browser::set_document_lang` to mark the page and `Phrases::store` to
//! remember the choice) and provides the new handle, which
//! re-renders every component reading it. A component deep in the tree
//! that changes the language, such as a picker, gets a `Callback<Lang>`
//! from the app like for any other app state.
//!
//! ```
//! # extern crate yew023 as yew;
//! use phrasebook::{Catalog, Lang, Phrasebook, Phrases, Source};
//! use phrasebook::yew023::use_phrases;
//! use yew::prelude::*;
//!
//! const SV_SE: Lang = Lang::new("sv-SE");
//!
//! static APP: Catalog = Catalog::new("app", &[
//!     Source::new(Lang::EN_US, "title = Mafia"),
//!     Source::new(SV_SE, "title = Maffia"),
//! ]);
//!
//! fn phrasebook() -> Phrasebook {
//!     Phrasebook::builder()
//!         .language(Lang::EN_US, "English")
//!         .language(SV_SE, "Svenska")
//!         .catalog(&APP)
//!         .build()
//! }
//!
//! enum Msg {
//!     SetLang(Lang),
//! }
//!
//! struct App {
//!     phrases: Phrases,
//! }
//!
//! impl Component for App {
//!     type Message = Msg;
//!     type Properties = ();
//!
//!     fn create(_: &Context<Self>) -> Self {
//!         // With the `browser` feature, start from the remembered or
//!         // preferred language: `browser::initial(&phrasebook())`.
//!         let phrases = phrasebook().phrases(Lang::EN_US);
//!         Self { phrases }
//!     }
//!
//!     fn update(&mut self, _: &Context<Self>, msg: Msg) -> bool {
//!         match msg {
//!             Msg::SetLang(lang) => {
//!                 self.phrases = self.phrases.with_lang(lang);
//!                 // With the `browser` feature:
//!                 // `browser::set_document_lang(lang); self.phrases.store();`
//!                 true
//!             }
//!         }
//!     }
//!
//!     fn view(&self, ctx: &Context<Self>) -> Html {
//!         let on_lang = ctx.link().callback(Msg::SetLang);
//!
//!         html! {
//!             <ContextProvider<Phrases> context={self.phrases.clone()}>
//!                 <Title />
//!                 <Picker {on_lang} />
//!             </ContextProvider<Phrases>>
//!         }
//!     }
//! }
//!
//! #[function_component]
//! fn Title() -> Html {
//!     let phrases = use_phrases();
//!     html! { <h1 lang={phrases.lang()}>{phrases.text("title")}</h1> }
//! }
//!
//! #[derive(Properties, PartialEq)]
//! struct PickerProps {
//!     on_lang: Callback<Lang>,
//! }
//!
//! #[function_component]
//! fn Picker(props: &PickerProps) -> Html {
//!     let phrases = use_phrases();
//!
//!     phrases
//!         .languages()
//!         .iter()
//!         .map(|language| {
//!             let lang = language.lang;
//!             let onclick = props.on_lang.reform(move |_| lang);
//!             html! { <button {onclick}>{language.name}</button> }
//!         })
//!         .collect()
//! }
//! ```
//!
//! # In a library
//!
//! A library ships its own [`Catalog`] under its prefix, which an app adds
//! to its phrasebook after its own catalog (so the app can override or
//! translate the library's messages). Each of the library's components
//! reads the handle through [`use_phrases_with`]: the provided handle with
//! the library's catalog as its backstop, so its messages render even when
//! the app did not add the catalog, or not in this language; or, with
//! nothing provided, the library's catalog alone in its default language.
//! An app that would rather render its own catalog than ids where nothing
//! is provided uses it with its own catalog as well.
//!
//! ```
//! # extern crate yew023 as yew;
//! use phrasebook::{Catalog, Lang, Source};
//! use phrasebook::yew023::use_phrases_with;
//! use yew::prelude::*;
//!
//! pub static L10N: Catalog = Catalog::new("watchword", &[
//!     Source::new(Lang::EN_US, "watchword-sign-in = Sign in"),
//! ])
//! .with_prefix("watchword-");
//!
//! #[function_component]
//! pub fn SignIn() -> Html {
//!     let phrases = use_phrases_with(&L10N);
//!     html! { <button>{phrases.text("watchword-sign-in")}</button> }
//! }
//! ```
//!
//! # In a struct component
//!
//! A struct component subscribes in `create` with a [`PhrasesContext`],
//! which it keeps: it stays subscribed while it lives, and later handles
//! arrive as a message, to pass to [`PhrasesContext::set`].
//!
//! Reading the handle in `view` with
//! `ctx.link().context::<Phrases>(Callback::noop())` does not follow
//! language changes: it subscribes nothing, so a component whose props did
//! not change is not rendered again when the root provides a new handle.
//!
//! ```
//! # extern crate yew023 as yew;
//! use phrasebook::{Catalog, Lang, Phrases, Source};
//! use phrasebook::yew023::PhrasesContext;
//! use yew::prelude::*;
//!
//! pub static L10N: Catalog = Catalog::new("watchword", &[
//!     Source::new(Lang::EN_US, "watchword-sign-out = Sign out"),
//! ])
//! .with_prefix("watchword-");
//!
//! pub enum Msg {
//!     Phrases(Phrases),
//! }
//!
//! pub struct SignOut {
//!     l10n: PhrasesContext,
//! }
//!
//! impl Component for SignOut {
//!     type Message = Msg;
//!     type Properties = ();
//!
//!     fn create(ctx: &Context<Self>) -> Self {
//!         // `PhrasesContext::new` in an app, without a catalog of its own.
//!         let l10n = PhrasesContext::new_with(ctx.link(), ctx.link().callback(Msg::Phrases), &L10N);
//!         Self { l10n }
//!     }
//!
//!     fn update(&mut self, _: &Context<Self>, msg: Msg) -> bool {
//!         match msg {
//!             Msg::Phrases(phrases) => self.l10n.set(phrases),
//!         }
//!     }
//!
//!     fn view(&self, _: &Context<Self>) -> Html {
//!         html! { <button>{self.l10n.phrases().text("watchword-sign-out")}</button> }
//!     }
//! }
//! ```
//!
//! [`ImplicitClone`]: yew023::html::ImplicitClone
//! [`IntoPropValue`]: yew023::html::IntoPropValue
//! [`Lang`]: crate::Lang

use std::cell::Cell;

use yew023::context::ContextHandle;
use yew023::functional::{Hook, HookContext, use_context};
use yew023::html::{BaseComponent, ImplicitClone, Scope};
use yew023::{Callback, Html};

use crate::{Args, Catalog, Language, Phrases};

/// Cheap to clone: an `Rc` and a few words.
impl ImplicitClone for Phrases {}

/// `Copy`.
impl ImplicitClone for Language {}

thread_local! {
    static WARNED: Cell<bool> = const { Cell::new(false) };
}

/// The provided handle, or [`Phrases::empty`], which renders ids, for a
/// component rendered where nothing provides one (a test, a portal, a
/// component used outside the app's root). The first fallback on each
/// thread is logged with `tracing`.
///
/// ```
/// use phrasebook::Phrases;
/// use phrasebook::yew023::or_default;
///
/// // What `use_context::<Phrases>()` returns outside a provider.
/// let phrases = or_default(None);
/// assert_eq!(&*phrases.text("title"), "title");
/// assert!(phrases == Phrases::empty());
/// ```
pub fn or_default(phrases: Option<Phrases>) -> Phrases {
    match phrases {
        Some(phrases) => phrases,
        None => {
            if !WARNED.replace(true) {
                tracing::warn!(
                    "phrasebook: no Phrases provided; rendering message ids (provide one with ContextProvider<Phrases>)"
                );
            }

            Phrases::empty()
        }
    }
}

/// For a library's components: the provided handle with `catalog` as its
/// backstop ([`Phrases::with_backstop`]), so a message the app's bundles
/// lack (the app did not add the catalog, or not in this language) comes
/// from the catalog; or, with nothing provided, [`Phrases::from_catalog`],
/// the catalog alone in its default language.
///
/// ```
/// use phrasebook::{Catalog, Lang, Phrasebook, Source};
/// use phrasebook::yew023::or_catalog;
///
/// static LIB: Catalog = Catalog::new("lib", &[
///     Source::new(Lang::EN_US, "lib-sign-in = Sign in"),
/// ])
/// .with_prefix("lib-");
///
/// // Nothing provided: the library's own English.
/// let phrases = or_catalog(None, &LIB);
/// assert_eq!(&*phrases.text("lib-sign-in"), "Sign in");
///
/// // Provided by an app that did not add the library's catalog.
/// let book = Phrasebook::builder().language(Lang::EN_US, "English").build();
/// let phrases = or_catalog(Some(book.phrases(Lang::EN_US)), &LIB);
/// assert_eq!(&*phrases.text("lib-sign-in"), "Sign in");
/// ```
pub fn or_catalog(phrases: Option<Phrases>, catalog: &'static Catalog) -> Phrases {
    match phrases {
        Some(phrases) => phrases.with_backstop(catalog),
        None => Phrases::from_catalog(catalog),
    }
}

/// The message `id` with `element` where the translation places the
/// argument `name`, such as a user's name in `<strong>` or a value in
/// `<code>`, from [`Phrases::msg_around`]. A translation that does not use
/// `name` gets `element` after its text, so the content is not lost.
///
/// ```
/// # extern crate yew023 as yew;
/// use phrasebook::{Args, Phrases};
/// use phrasebook::yew023::html_around;
/// use yew::prelude::*;
///
/// // removed = Removed { $user } from the team
/// fn removed(phrases: &Phrases, user: &str) -> Html {
///     html_around(phrases, "removed", &Args::new(), "user", html! { <strong>{user}</strong> })
/// }
/// ```
pub fn html_around(
    phrases: &Phrases,
    id: &str,
    args: &Args<'_>,
    name: &str,
    element: Html,
) -> Html {
    let around = phrases.msg_around(id, args, name);
    let mut nodes = Vec::with_capacity(3);

    if !around.before.is_empty() {
        nodes.push(Html::from(around.before));
    }

    nodes.push(element);

    if let Some(after) = around.after.filter(|after| !after.is_empty()) {
        nodes.push(Html::from(after));
    }

    nodes.into_iter().collect()
}

/// The provided handle, or [`Phrases::empty`] where nothing provides one,
/// as [`or_default`]. Re-renders the component when the root provides a new
/// handle.
///
/// ```
/// # extern crate yew023 as yew;
/// use phrasebook::yew023::use_phrases;
/// use yew::prelude::*;
///
/// #[function_component]
/// fn Title() -> Html {
///     let phrases = use_phrases();
///     html! { <h1>{phrases.text("title")}</h1> }
/// }
/// ```
pub fn use_phrases() -> impl Hook<Output = Phrases> {
    struct UsePhrases;

    impl Hook for UsePhrases {
        type Output = Phrases;

        fn run(self, ctx: &mut HookContext) -> Phrases {
            or_default(use_context::<Phrases>().run(ctx))
        }
    }

    UsePhrases
}

/// The provided handle with `catalog` as its backstop, or the catalog alone
/// where nothing provides one, as [`or_catalog`]: for a library's
/// components, or an app that would rather render its own catalog than ids
/// without a provider. Re-renders the component when the root provides a
/// new handle.
pub fn use_phrases_with(catalog: &'static Catalog) -> impl Hook<Output = Phrases> {
    struct UsePhrasesWith(&'static Catalog);

    impl Hook for UsePhrasesWith {
        type Output = Phrases;

        fn run(self, ctx: &mut HookContext) -> Phrases {
            or_catalog(use_context::<Phrases>().run(ctx), self.0)
        }
    }

    UsePhrasesWith(catalog)
}

/// A struct component's subscription to the provided handle: the current
/// handle, and the subscription, kept for as long as this lives. Create it
/// in `create`, and pass each handle the callback receives to
/// [`PhrasesContext::set`]. See [the module docs](self#in-a-struct-component).
pub struct PhrasesContext {
    phrases: Phrases,
    catalog: Option<&'static Catalog>,
    _handle: Option<ContextHandle<Phrases>>,
}

impl PhrasesContext {
    /// Subscribe to the provided handle; `callback` receives each later
    /// one. Where nothing provides a handle, [`Phrases::empty`], as
    /// [`or_default`].
    pub fn new<C>(link: &Scope<C>, callback: Callback<Phrases>) -> Self
    where
        C: BaseComponent,
    {
        Self::subscribe(link, callback, None)
    }

    /// As [`PhrasesContext::new`], with `catalog` as the backstop, or the
    /// catalog alone where nothing provides a handle, as [`or_catalog`].
    pub fn new_with<C>(
        link: &Scope<C>,
        callback: Callback<Phrases>,
        catalog: &'static Catalog,
    ) -> Self
    where
        C: BaseComponent,
    {
        Self::subscribe(link, callback, Some(catalog))
    }

    fn subscribe<C>(
        link: &Scope<C>,
        callback: Callback<Phrases>,
        catalog: Option<&'static Catalog>,
    ) -> Self
    where
        C: BaseComponent,
    {
        let (phrases, handle) = match link.context::<Phrases>(callback) {
            Some((phrases, handle)) => (Some(phrases), Some(handle)),
            None => (None, None),
        };

        Self {
            phrases: Self::resolve(phrases, catalog),
            catalog,
            _handle: handle,
        }
    }

    fn resolve(phrases: Option<Phrases>, catalog: Option<&'static Catalog>) -> Phrases {
        match catalog {
            Some(catalog) => or_catalog(phrases, catalog),
            None => or_default(phrases),
        }
    }

    /// The current handle.
    pub fn phrases(&self) -> &Phrases {
        &self.phrases
    }

    /// Take a handle the callback received, with the backstop applied.
    /// Returns whether it differs from the current one, for `update` to
    /// return.
    pub fn set(&mut self, phrases: Phrases) -> bool {
        let phrases = Self::resolve(Some(phrases), self.catalog);
        let changed = phrases != self.phrases;
        self.phrases = phrases;
        changed
    }
}
