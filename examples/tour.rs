//! A tour of phrasebook: an app's catalog, a library's catalog with a
//! backstop, plurals, and the fallback language.
//!
//! ```sh
//! cargo run --example tour
//! ```

use phrasebook::{Catalog, Lang, Phrasebook, Phrases, Source, args, check};

const SV_SE: Lang = Lang::new("sv-SE");

/// A library's catalog: every id under its prefix, English only.
mod auth {
    use phrasebook::{Catalog, Lang, Source};

    pub static L10N: Catalog = Catalog::new(
        "auth",
        &[Source::new(
            Lang::EN_US,
            "
auth-sign-in = Sign in
auth-sign-out = Sign out
auth-signed-in-as = Signed in as { $name }
",
        )],
    )
    .with_prefix("auth-");
}

/// The app's catalog, in English and partly in Swedish. The Swedish source
/// also translates one of the library's messages: the app's catalog is added
/// first, so it wins.
static APP: Catalog = Catalog::new(
    "app",
    &[
        Source::new(
            Lang::EN_US,
            "
title = Mafia
players = { $count ->
    [one] One player
   *[other] { $count } players
}
nights = { $count ->
    [0] No nights yet
    [one] One night
   *[other] { $count } nights
}
",
        ),
        Source::new(
            SV_SE,
            "
title = Maffia
players = { $count ->
    [one] En spelare
   *[other] { $count } spelare
}
auth-sign-in = Logga in
",
        ),
    ],
);

fn main() {
    let book = Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .language(SV_SE, "Svenska")
        .catalog(&APP)
        .catalog(&auth::L10N)
        .build();

    println!("Languages, for a picker:");

    for language in book.languages() {
        println!("  {} ({})", language.name(), language.lang());
    }

    println!("Starting language for a browser preferring de, sv-FI, en:");
    println!("  {}", book.negotiate(["de", "sv-FI", "en"]));

    for lang in [Lang::EN_US, SV_SE] {
        let phrases = book.phrases(lang);
        println!();
        println!("In {}:", phrases.lang());
        show(&phrases);
    }

    // A library used by an app that never added its catalog: with the
    // catalog as backstop, the library's messages still come from its own
    // English. This is what `yew023::or_catalog` does with a provided handle.
    let bare = Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .catalog(&APP)
        .build();

    let phrases = bare.phrases(Lang::EN_US);
    println!();
    println!("Without the library's catalog:");
    println!("  {}", phrases.text("auth-sign-in"));
    println!("With it as backstop:");
    println!(
        "  {}",
        phrases.with_backstop(&auth::L10N).text("auth-sign-in")
    );

    // With no phrasebook at all, the library renders its catalog alone, as
    // `yew023::or_catalog` does when nothing is provided.
    let phrases = Phrases::from_catalog(&auth::L10N);
    println!("With nothing provided:");
    println!(
        "  {}",
        phrases.msg("auth-signed-in-as", &args!(name = "Ada"))
    );

    // What an app's tests assert. Swedish also translates `auth-sign-in`,
    // which the app's English lacks: naming the library's catalog accepts
    // it, so parity reports only that Swedish lacks `nights`.
    println!();
    println!("Checks:");
    println!("  app catalog: {:?}", check::catalog(&APP).map(|()| "ok"));
    println!(
        "  library catalog: {:?}",
        check::catalog(&auth::L10N).map(|()| "ok")
    );

    if let Err(problems) = check::parity_with(&APP, SV_SE, &[&auth::L10N]) {
        println!("  sv-SE parity:");

        for problem in &problems {
            println!("    {problem}");
        }
    }
}

fn show(phrases: &Phrases) {
    println!("  {}", phrases.text("title"));

    for count in [1, 4] {
        println!("  {}", phrases.msg("players", &args!(count = count)));
    }

    // Not translated into Swedish: the fallback language's message.
    for count in [0, 1, 2] {
        println!("  {}", phrases.msg("nights", &args!(count = count)));
    }

    // A library message, translated by the app in Swedish.
    println!("  {}", phrases.text("auth-sign-in"));
    println!(
        "  {}",
        phrases.msg("auth-signed-in-as", &args!(name = "Ada"))
    );
    // Not defined anywhere: the id, so the hole is visible.
    println!("  {}", phrases.text("missing"));
}
