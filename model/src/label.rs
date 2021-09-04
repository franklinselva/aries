use aries_utils::input::Sym;

/// An optional user facing label to an object in the model.
/// In essence this is just an `Option<String>` with the string guaranteed to be non empty.
/// The added value comes from the various automatic conversions common label types.
/// TODO: refactor
#[derive(Clone)]
pub struct Label {
    pub(crate) lbl: Option<String>,
}
impl Label {
    pub fn new(str: String) -> Label {
        Label { lbl: Some(str) }
    }

    pub fn empty() -> Label {
        Label { lbl: None }
    }

    pub fn get(&self) -> Option<&str> {
        self.lbl.as_deref()
    }
}
impl From<Label> for Option<String> {
    fn from(lbl: Label) -> Self {
        lbl.lbl
    }
}
impl From<String> for Label {
    fn from(str: String) -> Self {
        if str.is_empty() {
            Label::empty()
        } else {
            Label::new(str)
        }
    }
}
impl From<&String> for Label {
    fn from(str: &String) -> Self {
        if str.is_empty() {
            Label::empty()
        } else {
            Label::new(str.clone())
        }
    }
}
impl From<&str> for Label {
    fn from(str: &str) -> Self {
        if str.is_empty() {
            Label::empty()
        } else {
            Label::new(str.into())
        }
    }
}

impl From<Sym> for Label {
    fn from(sym: Sym) -> Self {
        Label::from(sym.as_str())
    }
}
impl From<&Sym> for Label {
    fn from(sym: &Sym) -> Self {
        Label::from(sym.as_str())
    }
}

impl From<Option<String>> for Label {
    fn from(lbl: Option<String>) -> Self {
        match lbl {
            Some(lbl) => Label::from(lbl),
            None => Label::empty(),
        }
    }
}
impl<'a> From<&'a Option<String>> for Label {
    fn from(lbl: &'a Option<String>) -> Self {
        match lbl {
            Some(lbl) => Label::from(lbl.as_str()),
            None => Label::empty(),
        }
    }
}
impl<'a> From<Option<&'a str>> for Label {
    fn from(lbl: Option<&'a str>) -> Self {
        match lbl {
            Some(lbl) => Label::from(lbl),
            None => Label::empty(),
        }
    }
}
