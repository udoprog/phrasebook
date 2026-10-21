//! yew 0.23 trait impls on [`Lang`], so it passes as props and context like
//! yew's own cheap values and fills an element's `lang` attribute.

use alloc::rc::Rc;
use alloc::string::ToString;

use yew023::html::{ImplicitClone, IntoPropValue};
use yew023::virtual_dom::AttrValue;

use crate::Lang;

/// `Copy`.
impl ImplicitClone for Lang {}

/// The language's tag, such as `sv-SE`, for an element's `lang` attribute.
impl IntoPropValue<AttrValue> for Lang {
    fn into_prop_value(self) -> AttrValue {
        AttrValue::Rc(Rc::from(self.to_string()))
    }
}

/// The language's tag, such as `sv-SE`, for an element's `lang` attribute.
impl IntoPropValue<Option<AttrValue>> for Lang {
    fn into_prop_value(self) -> Option<AttrValue> {
        Some(self.into_prop_value())
    }
}

#[cfg(test)]
mod tests {
    use yew023::html::IntoPropValue;
    use yew023::virtual_dom::AttrValue;

    use crate::Lang;

    #[test]
    fn lang_attribute() {
        let value: AttrValue = Lang::new("sv-SE").into_prop_value();
        assert_eq!(&*value, "sv-SE");
        let value: Option<AttrValue> = Lang::EN_US.into_prop_value();
        assert_eq!(value.as_deref(), Some("en-US"));
    }
}
