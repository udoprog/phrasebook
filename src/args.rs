use std::borrow::Cow;
use std::fmt;
use std::rc::Rc;

use fluent_bundle::{FluentArgs, FluentValue};

/// The arguments of a message: named [`Value`]s for its variables.
///
/// Build them with [`args!`](crate::args!) for mixed types, [`args()`] for
/// same-typed pairs, or [`Args::with`] and [`Args::set`].
///
/// A variable a message selects a variant on that is passed as a string
/// spelling a number is formatted as that number, so `"3"` matches `[one]`
/// or `[3]` just as `3` does. See [`Phrases::msg`](crate::Phrases::msg).
///
/// # Examples
///
/// ```
/// use phrasebook::{Args, args};
///
/// let name = String::from("Ada");
///
/// let a = args!(count = 3, name = &name);
/// let b = Args::new().with("count", 3).with("name", &name);
/// let c = args([("count", "3"), ("name", "Ada")]);
/// # let _ = (a, b, c);
/// ```
#[derive(Default)]
pub struct Args<'a> {
    pub(crate) inner: FluentArgs<'a>,
}

impl<'a> Args<'a> {
    /// No arguments.
    pub fn new() -> Self {
        Args {
            inner: FluentArgs::new(),
        }
    }

    /// Set the variable `name` to `value`, replacing any earlier value.
    pub fn set(&mut self, name: &'a str, value: impl Into<Value<'a>>) -> &mut Self {
        self.inner.set(name, value.into().inner);
        self
    }

    /// [`Args::set`] by value, for chaining.
    pub fn with(mut self, name: &'a str, value: impl Into<Value<'a>>) -> Self {
        self.set(name, value);
        self
    }

    /// Whether any argument is a string, which is all that the selector
    /// number rule can apply to.
    pub(crate) fn has_strings(&self) -> bool {
        self.inner
            .iter()
            .any(|(_, value)| matches!(value, FluentValue::String(_)))
    }
}

impl fmt::Debug for Args<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.inner.iter()).finish()
    }
}

impl<'a, V> FromIterator<(&'a str, V)> for Args<'a>
where
    V: Into<Value<'a>>,
{
    fn from_iter<I: IntoIterator<Item = (&'a str, V)>>(iter: I) -> Self {
        let mut args = Args::new();

        for (name, value) in iter {
            args.set(name, value);
        }

        args
    }
}

/// Arguments from same-typed `(name, value)` pairs.
///
/// ```
/// use phrasebook::args;
///
/// let args = args([("count", 2), ("limit", 10)]);
/// # let _ = args;
/// ```
///
/// For values of different types, use [`args!`](crate::args!).
pub fn args<'a, V>(pairs: impl IntoIterator<Item = (&'a str, V)>) -> Args<'a>
where
    V: Into<Value<'a>>,
{
    pairs.into_iter().collect()
}

/// Arguments from `name = value` pairs of any types that convert into
/// [`Value`].
///
/// A name is an identifier or a string literal, the latter for Fluent
/// variable names with dashes.
///
/// ```
/// use phrasebook::args;
///
/// let name = String::from("Ada");
/// let points = 1.5;
///
/// let args = args!(pts = points, limit = 10, name = &name, "item-count" = "3");
/// # let _ = args;
/// ```
#[macro_export]
macro_rules! args {
    () => { $crate::Args::new() };
    ($($name:tt = $value:expr),+ $(,)?) => {{
        let mut args = $crate::Args::new();
        $(args.set($crate::__args_name!($name), $value);)+
        args
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __args_name {
    ($name:ident) => {
        stringify!($name)
    };
    ($name:literal) => {
        $name
    };
}

/// The value of a message argument: a number or a string.
///
/// Converts from every integer and float type, `&str`, `&String`, `String`,
/// `Cow<str>` and `Rc<str>`.
#[derive(Clone)]
pub struct Value<'a> {
    pub(crate) inner: FluentValue<'a>,
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            FluentValue::String(s) => fmt::Debug::fmt(s, f),
            FluentValue::Number(n) => fmt::Debug::fmt(&n.value, f),
            FluentValue::Custom(..) => f.write_str("Custom"),
            FluentValue::None => f.write_str("None"),
            FluentValue::Error => f.write_str("Error"),
        }
    }
}

macro_rules! number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for Value<'_> {
                #[inline]
                fn from(value: $ty) -> Self {
                    Value { inner: FluentValue::from(value) }
                }
            }

            impl From<&$ty> for Value<'_> {
                #[inline]
                fn from(value: &$ty) -> Self {
                    Value { inner: FluentValue::from(*value) }
                }
            }
        )*
    };
}

number!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);

impl<'a> From<&'a str> for Value<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Value {
            inner: FluentValue::from(value),
        }
    }
}

impl<'a> From<&'a String> for Value<'a> {
    #[inline]
    fn from(value: &'a String) -> Self {
        Value {
            inner: FluentValue::from(value.as_str()),
        }
    }
}

impl From<String> for Value<'_> {
    #[inline]
    fn from(value: String) -> Self {
        Value {
            inner: FluentValue::from(value),
        }
    }
}

impl<'a> From<Cow<'a, str>> for Value<'a> {
    #[inline]
    fn from(value: Cow<'a, str>) -> Self {
        Value {
            inner: FluentValue::String(value),
        }
    }
}

impl From<Rc<str>> for Value<'_> {
    #[inline]
    fn from(value: Rc<str>) -> Self {
        Value {
            inner: FluentValue::from(String::from(&*value)),
        }
    }
}

impl<'a> From<&'a Rc<str>> for Value<'a> {
    #[inline]
    fn from(value: &'a Rc<str>) -> Self {
        Value {
            inner: FluentValue::from(&**value),
        }
    }
}
