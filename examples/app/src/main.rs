//! A Yew app using phrasebook: the root owns the `Phrases` handle and
//! provides it, a picker changes the language and remembers it, and a
//! library-style component falls back to its own catalog.

use phrasebook::yew023::{or_catalog, or_default};
use phrasebook::{Catalog, Lang, Phrasebook, Phrases, Source, args, browser};
use yew::prelude::*;

const SV_SE: Lang = Lang::new("sv-SE");

static APP: Catalog = Catalog::new(
    "app",
    &[
        Source::new(
            Lang::EN_US,
            "
title = phrasebook example
language = Language
add-player = Add a player
players = { $count ->
    [0] No players yet
    [one] One player
   *[other] { $count } players
}
",
        ),
        Source::new(
            SV_SE,
            "
title = phrasebook-exempel
language = Språk
add-player = Lägg till en spelare
players = { $count ->
    [0] Inga spelare än
    [one] En spelare
   *[other] { $count } spelare
}
greeting-sign-in = Logga in
",
        ),
    ],
);

/// What a library would ship: its own catalog, under its prefix, in English.
static GREETING: Catalog = Catalog::new(
    "greeting",
    &[Source::new(
        Lang::EN_US,
        "
greeting-sign-in = Sign in
greeting-welcome = Welcome, { $name }!
",
    )],
)
.with_prefix("greeting-");

fn phrasebook() -> Phrasebook {
    Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .language(SV_SE, "Svenska")
        .catalog(&APP)
        .catalog(&GREETING)
        .build()
}

enum Msg {
    SetLang(Lang),
}

struct App {
    phrases: Phrases,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_: &Context<Self>) -> Self {
        let phrases = browser::initial(&phrasebook());
        Self { phrases }
    }

    fn update(&mut self, _: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetLang(lang) => {
                self.phrases = self.phrases.with_lang(lang);
                self.phrases.store();
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_lang = ctx.link().callback(Msg::SetLang);

        html! {
            <ContextProvider<Phrases> context={self.phrases.clone()}>
                <main lang={self.phrases.lang()}>
                    <h1>{self.phrases.text("title")}</h1>
                    <Picker {on_lang} />
                    <Players />
                    <Greeting name="Ada" />
                </main>
            </ContextProvider<Phrases>>
        }
    }
}

#[derive(Properties, PartialEq)]
struct PickerProps {
    on_lang: Callback<Lang>,
}

/// The app's language picker: it reads the handle and reports a choice
/// through the app's callback.
#[function_component]
fn Picker(props: &PickerProps) -> Html {
    let phrases = or_default(use_context::<Phrases>());

    let options = phrases.languages().iter().map(|language| {
        let onclick = props.on_lang.reform({
            let lang = language.lang;
            move |_| lang
        });

        html! {
            <button {onclick} disabled={language.lang == phrases.lang()}>
                {language.name}
            </button>
        }
    });

    html! {
        <p>{phrases.text("language")}{": "}{for options}</p>
    }
}

/// Plurals, with an argument.
#[function_component]
fn Players() -> Html {
    let phrases = or_default(use_context::<Phrases>());
    let count = use_state(|| 0u32);

    let onclick = {
        let count = count.clone();
        move |_| count.set(*count + 1)
    };

    html! {
        <p>
            {phrases.msg("players", &args!(count = *count))}
            {" "}
            <button {onclick}>{phrases.text("add-player")}</button>
        </p>
    }
}

#[derive(Properties, PartialEq)]
struct GreetingProps {
    name: AttrValue,
}

/// A library-style component: its messages come from the app's phrasebook
/// when it has them (the app translates `greeting-sign-in` into Swedish),
/// and from the library's own English otherwise.
#[function_component]
fn Greeting(props: &GreetingProps) -> Html {
    let phrases = or_catalog(use_context::<Phrases>(), &GREETING);

    html! {
        <p>
            {phrases.msg("greeting-welcome", &args!(name = props.name.as_str()))}
            {" "}
            <button>{phrases.text("greeting-sign-in")}</button>
        </p>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
