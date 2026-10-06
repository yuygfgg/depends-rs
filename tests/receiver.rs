use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Inner {
    value: &str,
}

#[lifetimes]
struct View {
    data: &str,
    inner: Inner,
}

#[lifetimes]
struct Holder<T> {
    value: T,
}

#[lifetimes]
struct ExplicitView<'a> {
    data: &'a str,
}

#[lifetimes]
impl View {
    #[depends(return = self.data)]
    fn data(&self) -> &str {
        self.data
    }

    #[depends(return = self.data)]
    fn data_mut(&mut self) -> &str {
        self.data
    }

    #[depends(return = self.inner.value)]
    fn nested(&self) -> &str {
        self.inner.value
    }

    #[depends(return = self.data)]
    fn into_data(self) -> &str {
        self.data
    }
}

#[lifetimes]
impl Holder<Inner> {
    #[depends(return = self.value.value)]
    fn generic_value(&self) -> &str {
        self.value.value
    }
}

#[lifetimes]
impl<'a> ExplicitView<'a> {
    #[depends(return = self.data)]
    fn data(&self) -> &str {
        self.data
    }
}

#[test]
fn receiver_fields_keep_stored_lifetimes_separate_from_receiver_borrows() {
    let data = String::from("data");
    let inner = String::from("inner");
    let mut view = View {
        data: &data,
        inner: Inner { value: &inner },
    };
    assert_eq!(view.data(), "data");
    assert_eq!(view.data_mut(), "data");
    assert_eq!(view.nested(), "inner");
    assert_eq!(view.into_data(), "data");

    let holder = Holder {
        value: Inner { value: &inner },
    };
    assert_eq!(holder.generic_value(), "inner");

    let explicit = ExplicitView { data: &data };
    assert_eq!(explicit.data(), "data");
}
