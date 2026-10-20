//! Components reading `Phrases` from yew's own context, rendered to a
//! string.

#![cfg(feature = "yew023")]

extern crate yew023 as yew;

use phrasebook::yew023::{html_around, or_catalog, or_default};
use phrasebook::{Args, Catalog, Lang, Phrasebook, Phrases, Source};
use yew::LocalServerRenderer;
use yew::context::ContextHandle;
use yew::html::{ImplicitClone, IntoPropValue};
use yew::prelude::*;
use yew::virtual_dom::AttrValue;

const SV_SE: Lang = Lang::new("sv-SE");

static APP: Catalog = Catalog::new(
    "app",
    &[
        Source::new(Lang::EN_US, "title = Mafia\nlib-sign-in = Log in"),
        Source::new(SV_SE, "title = Maffia"),
    ],
);

static LIB: Catalog = Catalog::new(
    "lib",
    &[
        Source::new(
            Lang::EN_US,
            "lib-sign-in = Sign in\nlib-sign-out = Sign out",
        ),
        Source::new(SV_SE, "lib-sign-out = Logga ut"),
    ],
)
.with_prefix("lib-");

fn phrases(lang: Lang) -> Phrases {
    Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .language(SV_SE, "Svenska")
        .catalog(&APP)
        .build()
        .phrases(lang)
}

#[function_component]
fn Title() -> Html {
    let phrases = or_default(use_context::<Phrases>());
    html! { <h1 lang={phrases.lang()}>{phrases.text("title")}</h1> }
}

#[function_component]
fn SignIn() -> Html {
    let phrases = or_catalog(use_context::<Phrases>(), &LIB);
    html! { <b>{phrases.text("lib-sign-in")}</b> }
}

enum Msg {
    Phrases(Phrases),
}

struct SignOut {
    phrases: Phrases,
    _handle: Option<ContextHandle<Phrases>>,
}

impl Component for SignOut {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let context = ctx
            .link()
            .context::<Phrases>(ctx.link().callback(Msg::Phrases));
        let phrases = or_catalog(context.as_ref().map(|(p, _)| p.clone()), &LIB);
        let _handle = context.map(|(_, handle)| handle);
        Self { phrases, _handle }
    }

    fn update(&mut self, _: &Context<Self>, msg: Msg) -> bool {
        match msg {
            Msg::Phrases(phrases) => {
                self.phrases = or_catalog(Some(phrases), &LIB);
                true
            }
        }
    }

    fn view(&self, _: &Context<Self>) -> Html {
        html! { <i>{self.phrases.text("lib-sign-out")}</i> }
    }
}

#[derive(Properties, PartialEq)]
struct RootProps {
    phrases: Option<Phrases>,
}

#[function_component]
fn Root(props: &RootProps) -> Html {
    let body = html! { <><Title /><SignIn /><SignOut /></> };

    match &props.phrases {
        Some(phrases) => html! {
            <ContextProvider<Phrases> context={phrases.clone()}>{body}</ContextProvider<Phrases>>
        },
        None => body,
    }
}

fn render(phrases: Option<Phrases>) -> String {
    let renderer = LocalServerRenderer::<Root>::with_props(RootProps { phrases }).hydratable(false);
    futures::executor::block_on(renderer.render())
}

#[test]
fn provided() {
    assert_eq!(
        render(Some(phrases(Lang::EN_US))),
        r#"<h1 lang="en-US">Mafia</h1><b>Log in</b><i>Sign out</i>"#
    );
}

#[test]
fn provided_other_language() {
    // The app overrides `lib-sign-in` in English only, so Swedish falls back
    // to the app's English; `lib-sign-out` comes from the library's Swedish.
    assert_eq!(
        render(Some(phrases(SV_SE))),
        r#"<h1 lang="sv-SE">Maffia</h1><b>Log in</b><i>Logga ut</i>"#
    );
}

#[test]
fn not_provided() {
    assert_eq!(
        render(None),
        r#"<h1 lang="en-US">title</h1><b>Sign in</b><i>Sign out</i>"#
    );
}

#[test]
fn helpers() {
    assert!(or_default(None) == Phrases::empty());
    let provided = phrases(SV_SE);
    assert!(or_default(Some(provided.clone())) == provided);
    assert!(or_catalog(Some(provided.clone()), &LIB) == provided.with_backstop(&LIB));
    assert!(or_catalog(None, &LIB) == Phrases::from_catalog(&LIB));
}

#[test]
fn traits() {
    let provided = phrases(SV_SE);
    assert!(provided.implicit_clone() == provided);
    assert_eq!(SV_SE.implicit_clone(), SV_SE);
    let value: AttrValue = SV_SE.into_prop_value();
    assert_eq!(&*value, "sv-SE");
    let value: Option<AttrValue> = Lang::EN_US.into_prop_value();
    assert_eq!(value.as_deref(), Some("en-US"));
}

#[test]
fn around() {
    static AROUND: Catalog = Catalog::new(
        "app",
        &[
            Source::new(
                Lang::EN_US,
                "removed = Removed { $user } from { $team }\nempty = Nobody\n",
            ),
            Source::new(SV_SE, "removed = { $user } togs bort\n"),
        ],
    );

    let book = Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .language(SV_SE, "Svenska")
        .catalog(&AROUND)
        .build();

    #[derive(Properties, PartialEq)]
    struct Props {
        phrases: Phrases,
        id: &'static str,
    }

    #[function_component]
    fn Removed(props: &Props) -> Html {
        let args = Args::new().with("team", "Red");
        html_around(
            &props.phrases,
            props.id,
            &args,
            "user",
            html! { <b>{"Ada"}</b> },
        )
    }

    let render = |lang, id| {
        let props = Props {
            phrases: book.phrases(lang),
            id,
        };
        let renderer = LocalServerRenderer::<Removed>::with_props(props).hydratable(false);
        futures::executor::block_on(renderer.render())
    };

    assert_eq!(
        render(Lang::EN_US, "removed"),
        "Removed <b>Ada</b> from Red"
    );
    assert_eq!(render(SV_SE, "removed"), "<b>Ada</b> togs bort");
    // Without the argument, the element follows the text.
    assert_eq!(render(Lang::EN_US, "empty"), "Nobody<b>Ada</b>");
}
