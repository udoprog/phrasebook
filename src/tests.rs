use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tracing::subscriber::with_default;
use tracing::{Event, Level, Metadata, Subscriber, span};

use crate::{Args, Catalog, Lang, Phrasebook, Phrases, Source, Value, args, check};

const SV_SE: Lang = Lang::new("sv-SE");
const DE_DE: Lang = Lang::new("de-DE");

static APP: Catalog = Catalog::new(
    "app",
    &[
        Source::new(
            Lang::EN_US,
            r#"
hello = Hello
greeting = Hello, { $name }!
only-english = Only in English
items = { $count ->
    [0] No items
    [one] One item
   *[other] { $count } items
}
nested = { $kind ->
    [a] { $count ->
        [one] One A
       *[other] Many A
    }
   *[other] Other
}
code = Code { $code }
mixed = { $pts } of { $limit } points
lib-sign-in = Log in
"#,
        ),
        Source::new(
            SV_SE,
            r#"
hello = Hej
greeting = Hej, { $name }!
items = { $count ->
    [0] Inga saker
    [one] En sak
   *[other] { $count } saker
}
"#,
        ),
        // Not declared by the phrasebooks below, so ignored.
        Source::new(DE_DE, "hello = Hallo\n"),
    ],
);

static LIB: Catalog = Catalog::new(
    "lib",
    &[
        Source::new(
            Lang::EN_US,
            "lib-sign-in = Sign in\nlib-delete = Delete { $name }\n",
        ),
        Source::new(SV_SE, "lib-sign-in = Logga in\n"),
    ],
)
.with_prefix("lib-");

fn book() -> Phrasebook {
    Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .language(SV_SE, "Svenska")
        .catalog(&APP)
        .catalog(&LIB)
        .build()
}

/// Counts the warnings logged while it is the default subscriber.
#[derive(Clone, Default)]
struct Warnings(Arc<AtomicUsize>);

impl Warnings {
    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl Subscriber for Warnings {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, event: &Event<'_>) {
        if *event.metadata().level() == Level::WARN {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

#[test]
fn lookup() {
    let book = book();
    let en = book.phrases(Lang::EN_US);
    let sv = book.phrases(SV_SE);

    assert_eq!(&*en.text("hello"), "Hello");
    assert_eq!(&*sv.text("hello"), "Hej");
    assert_eq!(en.msg("greeting", &args!(name = "Ada")), "Hello, Ada!");
    assert_eq!(sv.msg("greeting", &args!(name = "Ada")), "Hej, Ada!");
    assert_eq!(en.try_text("hello").as_deref(), Some("Hello"));
}

#[test]
fn text_is_cached() {
    let book = book();
    let en = book.phrases(Lang::EN_US);
    let a = en.text("hello");
    let b = en.text("hello");
    assert!(std::rc::Rc::ptr_eq(&a, &b));
}

#[test]
fn no_isolation_by_default() {
    let book = book();
    let en = book.phrases(Lang::EN_US);
    assert_eq!(en.msg("greeting", &args!(name = "Ada")), "Hello, Ada!");

    let isolating = Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .catalog(&APP)
        .use_isolating(true)
        .build();

    assert_eq!(
        isolating
            .phrases(Lang::EN_US)
            .msg("greeting", &args!(name = "Ada")),
        "Hello, \u{2068}Ada\u{2069}!"
    );
}

#[test]
fn arguments() {
    let book = book();
    let en = book.phrases(Lang::EN_US);
    let name = String::from("Ada");

    assert_eq!(en.msg("greeting", &args!(name = &name)), "Hello, Ada!");
    assert_eq!(
        en.msg("greeting", &args!(name = name.clone())),
        "Hello, Ada!"
    );
    assert_eq!(
        en.msg("greeting", &Args::new().with("name", "Ada")),
        "Hello, Ada!"
    );
    assert_eq!(en.msg("greeting", &args([("name", "Ada")])), "Hello, Ada!");

    let mut a = Args::new();
    a.set("name", std::rc::Rc::<str>::from("Ada"));
    assert_eq!(en.msg("greeting", &a), "Hello, Ada!");

    assert_eq!(
        en.msg("mixed", &args!(pts = 1.5, limit = 10u32)),
        "1.5 of 10 points"
    );
    assert_eq!(
        en.msg("mixed", &args!("pts" = 2, "limit" = "10",)),
        "2 of 10 points"
    );

    let collected: Args<'_> = [("name", Value::from("Ada"))].into_iter().collect();
    assert_eq!(en.msg("greeting", &collected), "Hello, Ada!");
}

#[test]
fn plurals_with_numbers_and_digit_strings() {
    let book = book();
    let en = book.phrases(Lang::EN_US);
    let sv = book.phrases(SV_SE);

    for (count, expected) in [(0, "No items"), (1, "One item"), (2, "2 items")] {
        assert_eq!(en.msg("items", &args!(count = count)), expected);
        let digits = count.to_string();
        assert_eq!(en.msg("items", &args!(count = &digits)), expected);
    }

    assert_eq!(sv.msg("items", &args!(count = "1")), "En sak");
    assert_eq!(sv.msg("items", &args!(count = 5)), "5 saker");

    // At any depth.
    assert_eq!(en.msg("nested", &args!(kind = "a", count = "1")), "One A");
    assert_eq!(en.msg("nested", &args!(kind = "a", count = 7)), "Many A");

    // Not a number: the default variant.
    assert_eq!(en.msg("items", &args!(count = "many")), "many items");
}

#[test]
fn unselected_digits_stay_as_written() {
    let book = book();
    let en = book.phrases(Lang::EN_US);
    assert_eq!(en.msg("code", &args!(code = "007")), "Code 007");
    // A selected variable is formatted as the number it spells.
    assert_eq!(en.msg("items", &args!(count = "02")), "2 items");
}

#[test]
fn fallback_language() {
    let book = book();
    let sv = book.phrases(SV_SE);

    assert_eq!(&*sv.text("only-english"), "Only in English");
    assert_eq!(
        sv.try_text("only-english").as_deref(),
        Some("Only in English")
    );

    // An explicit fallback.
    let book = Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .language(SV_SE, "Svenska")
        .fallback(SV_SE)
        .catalog(&APP)
        .build();

    assert_eq!(book.fallback(), SV_SE);
    assert_eq!(book.phrases(DE_DE).lang(), SV_SE);
    assert_eq!(&*book.phrases(Lang::EN_US).text("hello"), "Hello");
}

#[test]
fn undeclared_language_uses_the_fallback() {
    let book = book();
    let de = book.phrases(DE_DE);
    assert_eq!(de.lang(), Lang::EN_US);
    assert_eq!(&*de.text("hello"), "Hello");
    assert_eq!(book.phrases(SV_SE).with_lang(DE_DE).lang(), Lang::EN_US);
    assert!(!book.supports(DE_DE));
}

#[test]
fn missing_ids() {
    let book = book();
    let sv = book.phrases(SV_SE);

    assert_eq!(&*sv.text("no-such-id"), "no-such-id");
    assert_eq!(sv.msg("no-such-id", &args!(count = 1)), "no-such-id");
    assert_eq!(sv.try_text("no-such-id"), None);
    assert_eq!(sv.try_msg("no-such-id", &Args::new()), None);
}

#[test]
fn misses_are_logged_once() {
    let warnings = Warnings::default();

    with_default(warnings.clone(), || {
        let book = book();
        let sv = book.phrases(SV_SE);

        for _ in 0..3 {
            sv.text("no-such-id");
            sv.msg("no-such-id", &Args::new());
        }

        assert_eq!(warnings.count(), 1);

        for _ in 0..3 {
            sv.msg("only-english", &Args::new());
            sv.text("only-english");
        }

        assert_eq!(warnings.count(), 2);

        // Another language is logged on its own.
        book.phrases(Lang::EN_US).text("no-such-id");
        assert_eq!(warnings.count(), 3);

        // A missing argument: a format error, logged once per id.
        let en = book.phrases(Lang::EN_US);

        for _ in 0..3 {
            assert_eq!(en.msg("greeting", &Args::new()), "Hello, {$name}!");
        }

        assert_eq!(warnings.count(), 4);
    });
}

#[test]
fn library_catalog_fallback() {
    static APP_ONLY: Catalog = Catalog::new("app", &[Source::new(Lang::EN_US, "hello = Hello\n")]);

    let warnings = Warnings::default();

    with_default(warnings.clone(), || {
        // The app did not add the library's catalog.
        let book = Phrasebook::builder()
            .language(Lang::EN_US, "English")
            .language(SV_SE, "Svenska")
            .catalog(&APP_ONLY)
            .build();

        let en = book.phrases(Lang::EN_US).with_backstop(&LIB);
        assert_eq!(&*en.text("lib-sign-in"), "Sign in");
        assert_eq!(en.msg("lib-delete", &args!(name = "x")), "Delete x");
        assert_eq!(&*en.text("hello"), "Hello");

        // The library's own Swedish, and its default language after that.
        let sv = en.with_lang(SV_SE);
        assert_eq!(&*sv.text("lib-sign-in"), "Logga in");
        assert_eq!(sv.msg("lib-delete", &args!(name = "x")), "Delete x");

        // Found in the backstop: not a warning.
        assert_eq!(warnings.count(), 0);

        // Missing from the backstop too: the id.
        assert_eq!(&*sv.text("lib-nope"), "lib-nope");
        assert_eq!(sv.try_text("lib-nope"), None);
        assert_eq!(warnings.count(), 1);
    });

    // Without any phrasebook: the catalog alone.
    let lib = Phrases::from_catalog(&LIB);
    assert_eq!(lib.lang(), Lang::EN_US);
    assert_eq!(&*lib.text("lib-sign-in"), "Sign in");
    assert_eq!(&*lib.with_lang(SV_SE).text("lib-sign-in"), "Logga in");
    assert!(lib == Phrases::from_catalog(&LIB));

    // An app that added the catalog overrides it.
    let book = book();
    let en = book.phrases(Lang::EN_US).with_backstop(&LIB);
    assert_eq!(&*en.text("lib-sign-in"), "Log in");
    assert_eq!(en.msg("lib-delete", &args!(name = "x")), "Delete x");
}

#[test]
fn empty() {
    let empty = Phrases::empty();
    assert_eq!(&*empty.text("hello"), "hello");
    assert_eq!(empty.msg("hello", &args!(n = 1)), "hello");
    assert!(empty.languages().is_empty());
    assert!(empty == Phrases::empty());
    assert!(empty.with_backstop(&LIB) == Phrases::empty().with_backstop(&LIB));
    assert!(empty != Phrases::empty().with_backstop(&LIB));
    assert_eq!(&*empty.with_backstop(&LIB).text("lib-sign-in"), "Sign in");
}

#[test]
fn duplicate_id_precedence() {
    static FIRST: Catalog = Catalog::new("first", &[Source::new(Lang::EN_US, "a = First\n")]);
    static SECOND: Catalog = Catalog::new(
        "second",
        &[Source::new(Lang::EN_US, "a = Second\nb = Only second\n")],
    );
    static TWICE: Catalog =
        Catalog::new("twice", &[Source::new(Lang::EN_US, "c = One\nc = Two\n")]);

    let warnings = Warnings::default();

    with_default(warnings.clone(), || {
        let book = Phrasebook::builder()
            .language(Lang::EN_US, "English")
            .catalog(&FIRST)
            .catalog(&SECOND)
            .catalog(&TWICE)
            .build();

        let en = book.phrases(Lang::EN_US);
        assert_eq!(&*en.text("a"), "First");
        assert_eq!(&*en.text("b"), "Only second");
        assert_eq!(&*en.text("c"), "One");
        // `a` across catalogs and `c` within one.
        assert_eq!(warnings.count(), 2);
    });

    // An app overriding a library message inside its prefix is deliberate.
    let warnings = Warnings::default();

    with_default(warnings.clone(), || {
        let book = book();
        assert_eq!(&*book.phrases(Lang::EN_US).text("lib-sign-in"), "Log in");
        assert_eq!(warnings.count(), 0);
    });
}

#[test]
fn equality() {
    let book = book();
    let other = self::book();

    assert!(book.phrases(SV_SE) == book.phrases(SV_SE));
    assert!(book.phrases(SV_SE) != book.phrases(Lang::EN_US));
    assert!(book.phrases(SV_SE) != other.phrases(SV_SE));
    assert!(book.phrases(SV_SE).with_lang(Lang::EN_US) == book.phrases(Lang::EN_US));
    assert!(book == book.clone());
    assert!(book != other);
}

#[test]
fn languages_and_negotiation() {
    let book = book();
    let names = book
        .languages()
        .iter()
        .map(|l| (l.lang, l.name))
        .collect::<Vec<_>>();
    assert_eq!(names, [(Lang::EN_US, "English"), (SV_SE, "Svenska")]);
    assert_eq!(book.phrases(SV_SE).languages(), book.languages());
    assert_eq!(book.fallback(), Lang::EN_US);

    assert_eq!(book.negotiate(["sv-SE"]), SV_SE);
    assert_eq!(book.negotiate(["sv"]), SV_SE);
    assert_eq!(book.negotiate(["de", "sv"]), SV_SE);
    assert_eq!(book.negotiate(["en-GB", "sv"]), Lang::EN_US);
    assert_eq!(book.negotiate(["*", "de"]), Lang::EN_US);
    assert_eq!(book.negotiate([]), Lang::EN_US);
}

#[test]
fn bundles_parse_lazily() {
    // Swedish has a syntax error, logged only once Swedish is used.
    static BROKEN: Catalog = Catalog::new(
        "broken",
        &[
            Source::new(Lang::EN_US, "a = A\n"),
            Source::new(SV_SE, "a = A\n!junk\nc = C\n"),
        ],
    );

    let errors = Arc::new(AtomicUsize::new(0));

    #[derive(Clone)]
    struct Errors(Arc<AtomicUsize>);

    impl Subscriber for Errors {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(1)
        }

        fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

        fn event(&self, event: &Event<'_>) {
            if *event.metadata().level() == Level::ERROR {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        fn enter(&self, _: &span::Id) {}

        fn exit(&self, _: &span::Id) {}
    }

    with_default(Errors(errors.clone()), || {
        let book = Phrasebook::builder()
            .language(Lang::EN_US, "English")
            .language(SV_SE, "Svenska")
            .catalog(&BROKEN)
            .build();

        assert_eq!(&*book.phrases(Lang::EN_US).text("a"), "A");
        assert_eq!(errors.load(Ordering::SeqCst), 0);

        // What parsed is kept.
        let sv = book.phrases(SV_SE);
        assert_eq!(&*sv.text("a"), "A");
        assert_eq!(&*sv.text("c"), "C");
        assert_eq!(errors.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn check_catalog() {
    check::catalog(&APP).unwrap();
    check::catalog(&LIB).unwrap();

    static BAD: Catalog = Catalog::new(
        "bad",
        &[
            Source::new(
                Lang::EN_US,
                "bad-a = A\nbad-a = Again\nb = B\nbad-c = { $x } { missing-ref }\n!junk\n-bad-term = T\n-term = T\n",
            ),
            Source::new(Lang::EN_US, "bad-a = A\n"),
        ],
    )
    .with_prefix("bad-");

    let problems = check::catalog(&BAD).unwrap_err();
    let messages = problems.iter().map(|p| p.message()).collect::<Vec<_>>();

    assert_eq!(
        messages,
        [
            "syntax error on line 5: Expected one of \"a-zA-Z\"",
            "`bad-a` is defined twice",
            "`b` does not start with the catalog's prefix `bad-`",
            "`-term` does not start with the catalog's prefix `bad-`",
            "`bad-c` does not format: Resolver error: Unknown message: missing-ref",
            "a second source for this language, which is ignored",
        ],
        "{problems}"
    );

    assert!(problems.iter().all(|p| p.catalog() == "bad"));
    assert!(problems.iter().all(|p| p.lang() == Lang::EN_US));
}

#[test]
fn check_parity_and_ids() {
    assert_eq!(check::ids(&APP, SV_SE), ["hello", "greeting", "items"]);
    assert!(check::ids(&APP, Lang::new("fr")).is_empty());

    let problems = check::parity(&APP, SV_SE).unwrap_err();
    let messages = problems.iter().map(|p| p.message()).collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "`only-english` is missing (defined in en-US)",
            "`nested` is missing (defined in en-US)",
            "`code` is missing (defined in en-US)",
            "`mixed` is missing (defined in en-US)",
            "`lib-sign-in` is missing (defined in en-US)",
        ]
    );

    let problems = check::parity(&APP, DE_DE).unwrap_err();
    assert_eq!(
        problems.iter().map(|p| p.message()).collect::<Vec<_>>(),
        [
            "`greeting` is missing (defined in en-US)",
            "`only-english` is missing (defined in en-US)",
            "`items` is missing (defined in en-US)",
            "`nested` is missing (defined in en-US)",
            "`code` is missing (defined in en-US)",
            "`mixed` is missing (defined in en-US)",
            "`lib-sign-in` is missing (defined in en-US)",
        ]
    );

    static EXTRA: Catalog = Catalog::new(
        "extra",
        &[
            Source::new(Lang::EN_US, "a = A\n"),
            Source::new(SV_SE, "a = A\nb = B\n"),
        ],
    );

    let problems = check::parity(&EXTRA, SV_SE).unwrap_err();
    assert_eq!(
        problems.to_string(),
        "extra sv-SE: `b` is not defined in en-US"
    );

    assert!(check::parity(&LIB, Lang::new("fr")).is_err());
}

#[test]
fn check_parity_with_libraries() {
    // The app translates one of the library's messages, misspells another,
    // and has a stray id of its own.
    static TRANSLATING: Catalog = Catalog::new(
        "app",
        &[
            Source::new(Lang::EN_US, "a = A\nb = B\n"),
            Source::new(
                SV_SE,
                "a = A\nlib-sign-in = Logga in\nlib-sign-up = Registrera\nc = C\n",
            ),
        ],
    );

    // Without libraries, parity reports every id English lacks, and
    // parity_with with none says the same.
    let problems = check::parity(&TRANSLATING, SV_SE).unwrap_err();
    assert_eq!(
        problems.to_string(),
        "\
app sv-SE: `b` is missing (defined in en-US)
app sv-SE: `lib-sign-in` is not defined in en-US
app sv-SE: `lib-sign-up` is not defined in en-US
app sv-SE: `c` is not defined in en-US"
    );
    assert_eq!(
        check::parity_with(&TRANSLATING, SV_SE, &[]).unwrap_err(),
        problems
    );

    // With the library, its id is accepted; an unknown id under its prefix
    // and the stray id are still reported, and the library's other ids are
    // not required.
    let problems = check::parity_with(&TRANSLATING, SV_SE, &[&LIB]).unwrap_err();
    assert_eq!(
        problems.to_string(),
        "\
app sv-SE: `b` is missing (defined in en-US)
app sv-SE: `lib-sign-up` is defined neither in en-US nor in any given library
app sv-SE: `c` is defined neither in en-US nor in any given library"
    );

    // Only a library's default language counts: an id the library defines
    // in Swedish alone is not accepted.
    static PARTIAL: Catalog = Catalog::new(
        "lib",
        &[
            Source::new(Lang::EN_US, "lib-sign-in = Sign in\n"),
            Source::new(SV_SE, "lib-sign-in = Logga in\nlib-sign-up = Registrera\n"),
        ],
    )
    .with_prefix("lib-");

    let problems = check::parity_with(&TRANSLATING, SV_SE, &[&PARTIAL]).unwrap_err();
    assert_eq!(
        problems,
        check::parity_with(&TRANSLATING, SV_SE, &[&LIB]).unwrap_err()
    );

    // Missing ids are reported as before.
    let problems = check::parity_with(&APP, SV_SE, &[&LIB]).unwrap_err();
    assert_eq!(problems, check::parity(&APP, SV_SE).unwrap_err());
}

#[test]
fn check_all() {
    // One report: the catalog's own problems, then parity in each other
    // language, once per language even with a second source for it.
    static ALL: Catalog = Catalog::new(
        "app",
        &[
            Source::new(Lang::EN_US, "a = A\nb = B\nb = Again\n"),
            Source::new(SV_SE, "a = A\nlib-sign-in = Logga in\n"),
            Source::new(DE_DE, "a = A\nb = B\nc = C\n"),
            Source::new(SV_SE, "a = A\n"),
        ],
    );

    let problems = check::all(&ALL, &[&LIB]).unwrap_err();
    assert_eq!(
        problems.to_string(),
        "\
app en-US: `b` is defined twice
app sv-SE: a second source for this language, which is ignored
app sv-SE: `b` is missing (defined in en-US)
app de-DE: `c` is defined neither in en-US nor in any given library"
    );

    assert_eq!(check::all_ids(&ALL), ["a", "b", "lib-sign-in", "c"]);
}

#[test]
fn has_catalog() {
    let book = Phrasebook::builder()
        .language(Lang::EN_US, "English")
        .catalog(&APP)
        .build();

    assert!(book.has_catalog(&APP));
    assert!(!book.has_catalog(&LIB));
}

#[cfg(feature = "koine01")]
#[test]
fn koine01_warns_about_unknown_languages() {
    let warnings = Warnings::default();

    with_default(warnings.clone(), || {
        let _ = Phrasebook::builder()
            .language(Lang::EN_US, "English")
            .language(Lang::new("sv"), "Svenska")
            .language(Lang::new("fil-PH"), "Filipino")
            .build();
    });

    assert_eq!(warnings.count(), 0);

    with_default(warnings.clone(), || {
        let _ = Phrasebook::builder()
            .language(Lang::EN_US, "English")
            .language(Lang::new("xx"), "Unknown")
            .language(Lang::new("sv-JP"), "Svenska")
            .language(Lang::new("en-ZZ"), "English")
            .build();
    });

    assert_eq!(warnings.count(), 3);
}

#[cfg(feature = "koine01")]
#[test]
fn koine01_language_named_by_koine() {
    let warnings = Warnings::default();

    let book = with_default(warnings.clone(), || {
        Phrasebook::builder()
            .language_named_by_koine(SV_SE)
            .language_named_by_koine(Lang::new("fil"))
            .language_named_by_koine(Lang::new("xx-YY"))
            // Already declared: keeps its name.
            .language_named_by_koine(SV_SE)
            .build()
    });

    let names = book
        .languages()
        .iter()
        .map(|l| (l.lang, l.name))
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        [
            (SV_SE, "Swedish"),
            (Lang::new("fil"), "Filipino"),
            (Lang::new("xx-YY"), "xx-YY"),
        ]
    );
    assert_eq!(book.fallback(), SV_SE);
    assert_eq!(warnings.count(), 1);
}
