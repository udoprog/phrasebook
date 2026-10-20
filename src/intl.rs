//! Numbers formatted by the browser's `Intl.NumberFormat`, in the bundle's
//! language.

use std::cell::RefCell;
use std::collections::HashMap;

use fluent_bundle::FluentValue;
use fluent_bundle::types::{
    FluentNumber, FluentNumberCurrencyDisplayStyle, FluentNumberOptions, FluentNumberStyle,
};
use intl_memoizer::{IntlLangMemoizer, Memoizable};
use js_sys::{Array, Function, Object, Reflect};
use unic_langid::LanguageIdentifier;
use wasm_bindgen::{JsCast, JsValue};

use crate::Lang;

/// A bound `Intl.NumberFormat` `format` function for one language and set
/// of options, or `None` when the browser refused them (an unknown currency,
/// say), which leaves the number to fluent.
struct NumberFormat(Option<Function>);

impl Memoizable for NumberFormat {
    type Args = (FluentNumberOptions,);
    type Error = ();

    fn construct(lang: LanguageIdentifier, (options,): Self::Args) -> Result<Self, ()> {
        Ok(NumberFormat(new_format(&lang.to_string(), &options)))
    }
}

fn new_format(tag: &str, options: &FluentNumberOptions) -> Option<Function> {
    let intl = Reflect::get(&js_sys::global(), &"Intl".into()).ok()?;
    let constructor = Reflect::get(&intl, &"NumberFormat".into())
        .ok()?
        .dyn_into::<Function>()
        .ok()?;

    let object = Object::new();

    let style = match options.style {
        FluentNumberStyle::Decimal => "decimal",
        FluentNumberStyle::Currency => "currency",
        FluentNumberStyle::Percent => "percent",
    };

    set(&object, "style", &style.into());

    if let Some(currency) = &options.currency {
        set(&object, "currency", &currency.into());
    }

    let currency_display = match options.currency_display {
        FluentNumberCurrencyDisplayStyle::Symbol => "symbol",
        FluentNumberCurrencyDisplayStyle::Code => "code",
        FluentNumberCurrencyDisplayStyle::Name => "name",
    };

    set(&object, "currencyDisplay", &currency_display.into());
    set(&object, "useGrouping", &options.use_grouping.into());

    let digits = [
        ("minimumIntegerDigits", options.minimum_integer_digits),
        ("minimumFractionDigits", options.minimum_fraction_digits),
        ("maximumFractionDigits", options.maximum_fraction_digits),
        (
            "minimumSignificantDigits",
            options.minimum_significant_digits,
        ),
        (
            "maximumSignificantDigits",
            options.maximum_significant_digits,
        ),
    ];

    for (key, value) in digits {
        if let Some(value) = value {
            set(&object, key, &(value as f64).into());
        }
    }

    // Through `Reflect`, so options the browser rejects are an error here
    // rather than a thrown exception.
    let arguments = Array::of2(&tag.into(), &object);
    let format = Reflect::construct(&constructor, &arguments).ok()?;

    Reflect::get(&format, &"format".into())
        .ok()?
        .dyn_into::<Function>()
        .ok()
}

fn set(object: &Object, key: &str, value: &JsValue) {
    let _ = Reflect::set(object, &key.into(), value);
}

fn format_with(intls: &IntlLangMemoizer, number: &FluentNumber) -> Option<String> {
    intls
        .with_try_get::<NumberFormat, _, _>((number.options.clone(),), |format| {
            let format = format.0.as_ref()?;
            format
                .call1(&JsValue::UNDEFINED, &JsValue::from_f64(number.value))
                .ok()?
                .as_string()
        })
        .ok()
        .flatten()
}

/// The bundle formatter: numbers through `Intl.NumberFormat`, anything else
/// left to fluent.
pub(crate) fn format(value: &FluentValue<'_>, intls: &IntlLangMemoizer) -> Option<String> {
    match value {
        FluentValue::Number(number) => format_with(intls, number),
        _ => None,
    }
}

thread_local! {
    static MEMOIZERS: RefCell<HashMap<Lang, IntlLangMemoizer>> = RefCell::default();
}

/// A number outside a message, in `lang`.
pub(crate) fn number(lang: Lang, number: &FluentNumber) -> Option<String> {
    MEMOIZERS.with_borrow_mut(|memoizers| {
        let intls = memoizers.entry(lang).or_insert_with(|| {
            let langid = lang.to_string().parse().unwrap_or_default();
            IntlLangMemoizer::new(langid)
        });

        format_with(intls, number)
    })
}
