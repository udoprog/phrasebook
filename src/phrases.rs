use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::{Args, Catalog, Lang, Language, Phrasebook};

/// The handle components use: a [`Phrasebook`] open at one language.
///
/// Cloning is cheap (an `Rc` and a few words), and two handles compare
/// equal when they share a phrasebook, a language and a backstop, so a
/// component tree handed one re-renders only when the language changes.
///
/// # Lookup
///
/// A message is formatted in the first of these that defines it:
///
/// 1. the handle's language;
/// 2. the phrasebook's fallback language;
/// 3. the handle's backstop catalog, if it has one
///    ([`Phrases::with_backstop`]): in the handle's language if the catalog
///    has a source for it, else in the catalog's default language;
/// 4. otherwise the result is the id itself, so the hole is visible on the
///    page rather than blank.
///
/// Each miss (the handle's language lacking a message, or every bundle
/// lacking it) is logged once per id and language with `tracing`, and
/// format errors (a missing argument, say) once per id, so a page that
/// re-renders often does not drown the console.
///
/// # Examples
///
/// ```
/// use phrasebook::{Catalog, Lang, Phrasebook, Source, args};
///
/// static APP: Catalog = Catalog::new("app", &[
///     Source::new(Lang::EN_US, r#"
/// greeting = Hello, { $name }!
/// items = { $count ->
///     [one] One item
///    *[other] { $count } items
/// }
/// "#),
/// ]);
///
/// let book = Phrasebook::builder()
///     .language(Lang::EN_US, "English")
///     .catalog(&APP)
///     .build();
///
/// let phrases = book.phrases(Lang::EN_US);
///
/// assert_eq!(phrases.msg("greeting", &args!(name = "Ada")), "Hello, Ada!");
/// assert_eq!(phrases.msg("items", &args!(count = 1)), "One item");
/// // A count passed as its digits selects the same variant.
/// assert_eq!(phrases.msg("items", &args!(count = "1")), "One item");
/// assert_eq!(phrases.msg("items", &args!(count = 3)), "3 items");
/// ```
#[derive(Clone)]
pub struct Phrases {
    book: Phrasebook,
    lang: Lang,
    backstop: Option<Backstop>,
}

#[derive(Clone)]
struct Backstop {
    catalog: &'static Catalog,
    book: Phrasebook,
}

thread_local! {
    static EMPTY: Phrasebook = Phrasebook::builder().build();
    static CATALOGS: RefCell<Vec<(usize, Phrasebook)>> = const { RefCell::new(Vec::new()) };
}

/// The phrasebook of `catalog` alone, built once per thread: every language
/// it has a source for, its default language as the fallback.
fn catalog_book(catalog: &'static Catalog) -> Phrasebook {
    let address = catalog as *const Catalog as usize;

    CATALOGS.with(|books| {
        if let Some((_, book)) = books.borrow().iter().find(|(a, _)| *a == address) {
            return book.clone();
        }

        let mut builder = Phrasebook::builder();

        for lang in catalog.languages() {
            builder = builder.language(lang, "");
        }

        let book = builder.catalog(catalog).build();
        books.borrow_mut().push((address, book.clone()));
        book
    })
}

impl Phrases {
    pub(crate) fn new(book: Phrasebook, lang: Lang) -> Phrases {
        Phrases {
            book,
            lang,
            backstop: None,
        }
    }

    /// A handle with no catalogs, so every lookup renders the id; shared per
    /// thread, so every call returns an equal handle. For a component used
    /// where nothing provides a handle.
    ///
    /// ```
    /// use phrasebook::Phrases;
    ///
    /// let phrases = Phrases::empty();
    /// assert_eq!(&*phrases.text("hello"), "hello");
    /// assert!(phrases == Phrases::empty());
    /// ```
    pub fn empty() -> Phrases {
        let book = EMPTY.with(Phrasebook::clone);
        let lang = book.fallback();
        Phrases::new(book, lang)
    }

    /// `catalog` alone, in its default language (that of its first source),
    /// built once per thread per catalog: what a library renders when
    /// nothing provides a handle. Its other languages are reachable with
    /// [`Phrases::with_lang`]; their [`Language::name`]s are empty.
    ///
    /// ```
    /// use phrasebook::{Catalog, Lang, Phrases, Source};
    ///
    /// static LIB: Catalog = Catalog::new("lib", &[
    ///     Source::new(Lang::EN_US, "lib-hello = Hello"),
    /// ])
    /// .with_prefix("lib-");
    ///
    /// let phrases = Phrases::from_catalog(&LIB);
    /// assert_eq!(phrases.lang(), Lang::EN_US);
    /// assert_eq!(&*phrases.text("lib-hello"), "Hello");
    /// assert!(phrases == Phrases::from_catalog(&LIB));
    /// ```
    pub fn from_catalog(catalog: &'static Catalog) -> Phrases {
        let book = catalog_book(catalog);
        let lang = book.fallback();
        Phrases::new(book, lang)
    }

    /// The same handle, consulting `catalog` when every bundle in the chain
    /// lacks a message (see [Lookup](Phrases#lookup)).
    ///
    /// A library component uses this on the handle its app provides, so its
    /// own messages render even when the app did not add its catalog, or
    /// not in this language. A message found only in the backstop is not
    /// logged as missing.
    ///
    /// ```
    /// use phrasebook::{Catalog, Lang, Phrasebook, Source};
    ///
    /// static APP: Catalog = Catalog::new("app", &[
    ///     Source::new(Lang::EN_US, "hello = Hello"),
    /// ]);
    ///
    /// static LIB: Catalog = Catalog::new("lib", &[
    ///     Source::new(Lang::EN_US, "lib-sign-in = Sign in"),
    /// ])
    /// .with_prefix("lib-");
    ///
    /// // The app did not add the library's catalog.
    /// let book = Phrasebook::builder()
    ///     .language(Lang::EN_US, "English")
    ///     .catalog(&APP)
    ///     .build();
    ///
    /// let phrases = book.phrases(Lang::EN_US).with_backstop(&LIB);
    /// assert_eq!(&*phrases.text("lib-sign-in"), "Sign in");
    /// assert_eq!(&*phrases.text("hello"), "Hello");
    /// ```
    pub fn with_backstop(&self, catalog: &'static Catalog) -> Phrases {
        Phrases {
            book: self.book.clone(),
            lang: self.lang,
            backstop: Some(Backstop {
                catalog,
                book: catalog_book(catalog),
            }),
        }
    }

    /// The handle's language.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// The phrasebook's declared languages, for a language picker.
    pub fn languages(&self) -> &[Language] {
        self.book.languages()
    }

    /// The phrasebook this handle reads.
    pub fn phrasebook(&self) -> &Phrasebook {
        &self.book
    }

    /// The same bundles (and backstop) in another language, or in the
    /// fallback language if `lang` is not declared. An app's root calls
    /// this on a language change and provides the result.
    pub fn with_lang(&self, lang: Lang) -> Phrases {
        Phrases {
            book: self.book.clone(),
            lang: self.book.resolve(lang),
            backstop: self.backstop.clone(),
        }
    }

    /// A message without arguments, or its id if nothing defines it.
    ///
    /// Resolved once per language and id and handed out as an `Rc` clone
    /// after that.
    pub fn text(&self, id: &str) -> Rc<str> {
        match self.try_text(id) {
            Some(text) => text,
            None => {
                self.missing(id);
                Rc::from(id)
            }
        }
    }

    /// A message with arguments, or its id if nothing defines it.
    ///
    /// A variable the message selects a variant on (`{ $count -> ... }`, at
    /// any depth) that is passed as a string spelling a number is formatted
    /// as that number, so `[one]` and `[0]` match whether the caller passed
    /// `3` or `"3"`. Other variables are left as written.
    pub fn msg(&self, id: &str, args: &Args<'_>) -> String {
        match self.try_msg(id, args) {
            Some(text) => text,
            None => {
                self.missing(id);
                id.to_owned()
            }
        }
    }

    /// [`Phrases::text`] without the fallback to the id: `None` when nothing
    /// in the chain defines it.
    pub fn try_text(&self, id: &str) -> Option<Rc<str>> {
        if let Some(text) = self.book.inner.text(self.lang, id, false) {
            return Some(text);
        }

        let backstop = self.backstop.as_ref()?;
        let lang = backstop.book.resolve(self.lang);
        let text = backstop.book.inner.text(lang, id, true)?;
        self.backstopped(id, backstop);
        Some(text)
    }

    /// [`Phrases::msg`] without the fallback to the id: `None` when nothing
    /// in the chain defines it.
    pub fn try_msg(&self, id: &str, args: &Args<'_>) -> Option<String> {
        if let Some(text) = self.book.inner.format(self.lang, id, Some(args), false) {
            return Some(text);
        }

        let backstop = self.backstop.as_ref()?;
        let lang = backstop.book.resolve(self.lang);
        let text = backstop.book.inner.format(lang, id, Some(args), true)?;
        self.backstopped(id, backstop);
        Some(text)
    }

    /// A message split where the translation places the argument `name`,
    /// for markup in its place, such as a user's name in `<strong>`: the
    /// text before it and, if the message uses `name`, the text after it.
    /// The other arguments are formatted as in [`Phrases::msg`], and a
    /// value `args` gives for `name` is ignored. If the message uses `name`
    /// more than once, the split is at the first use and later uses are
    /// left out.
    ///
    /// ```
    /// use phrasebook::{Args, Catalog, Lang, Phrasebook, Source, args};
    ///
    /// const SV_SE: Lang = Lang::new("sv-SE");
    ///
    /// static APP: Catalog = Catalog::new("app", &[
    ///     Source::new(Lang::EN_US, "removed = Removed { $user } from { $team }\nempty = Nobody here\n"),
    ///     Source::new(SV_SE, "removed = { $user } togs bort från { $team }\n"),
    /// ]);
    ///
    /// let book = Phrasebook::builder()
    ///     .language(Lang::EN_US, "English")
    ///     .language(SV_SE, "Svenska")
    ///     .catalog(&APP)
    ///     .build();
    ///
    /// let around = book.phrases(SV_SE).msg_around("removed", &args!(team = "Röd"), "user");
    /// assert_eq!(around.before, "");
    /// assert_eq!(around.after.as_deref(), Some(" togs bort från Röd"));
    ///
    /// // A message without the argument.
    /// let around = book.phrases(Lang::EN_US).msg_around("empty", &Args::new(), "user");
    /// assert_eq!(around.before, "Nobody here");
    /// assert_eq!(around.after, None);
    /// ```
    pub fn msg_around(&self, id: &str, args: &Args<'_>, name: &str) -> Around {
        // A private use character, which no message contains.
        const MARK: &str = "\u{E000}";
        // Bidi isolation around an argument, with `use_isolating`.
        const ISOLATES: [char; 2] = ['\u{2068}', '\u{2069}'];
        const ISOLATED: &str = "\u{2068}\u{E000}\u{2069}";

        let mut marked = Args::new();

        for (key, value) in args.inner.iter() {
            if key != name {
                marked.inner.set(key, value.clone());
            }
        }

        marked.inner.set(name, MARK);
        let text = self.msg(id, &marked);

        match text.split_once(MARK) {
            Some((before, after)) => Around {
                before: before
                    .strip_suffix(ISOLATES[0])
                    .unwrap_or(before)
                    .to_owned(),
                after: Some(
                    after
                        .strip_prefix(ISOLATES[1])
                        .unwrap_or(after)
                        .replace(ISOLATED, "")
                        .replace(MARK, ""),
                ),
            },
            None => Around {
                before: text,
                after: None,
            },
        }
    }

    fn missing(&self, id: &str) {
        if self.book.inner.first_miss(self.lang, id) {
            tracing::warn!(lang = %self.lang, id, "phrasebook: message missing");
        }
    }

    fn backstopped(&self, id: &str, backstop: &Backstop) {
        if self.book.inner.first_miss(self.lang, id) {
            tracing::debug!(
                lang = %self.lang,
                id,
                catalog = backstop.catalog.name(),
                "phrasebook: message from the backstop catalog"
            );
        }
    }
}

/// A message split where the translation places an argument, from
/// [`Phrases::msg_around`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Around {
    /// The text before the argument, or the whole message if it does not
    /// use the argument.
    pub before: String,
    /// The text after the argument; `None` if the message does not use it.
    pub after: Option<String>,
}

#[cfg(feature = "browser")]
#[cfg_attr(phrasebook_docsrs, doc(cfg(feature = "browser")))]
impl Phrases {
    /// Remember the handle's language in localStorage under
    /// [`browser::DEFAULT_KEY`](crate::browser::DEFAULT_KEY), for
    /// [`browser::initial`](crate::browser::initial) to pick up on the next
    /// visit. Outside wasm it does nothing.
    pub fn store(&self) {
        self.store_as(crate::browser::DEFAULT_KEY);
    }

    /// [`Phrases::store`] under another key.
    pub fn store_as(&self, key: &str) {
        crate::browser::store(self.lang, key);
    }
}

/// Equal when they share a phrasebook (by pointer), a language and a
/// backstop catalog (by pointer).
impl PartialEq for Phrases {
    fn eq(&self, other: &Self) -> bool {
        self.lang == other.lang
            && self.book.ptr_eq(&other.book)
            && match (&self.backstop, &other.backstop) {
                (None, None) => true,
                (Some(a), Some(b)) => core::ptr::eq(a.catalog, b.catalog),
                _ => false,
            }
    }
}

impl fmt::Debug for Phrases {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Phrases")
            .field("lang", &self.lang)
            .field("phrasebook", &self.book)
            .field(
                "backstop",
                &self.backstop.as_ref().map(|b| b.catalog.name()),
            )
            .finish()
    }
}
