#![allow(clippy::needless_lifetimes)]

use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

trait Reader {
    #[depends(return.data = text)]
    fn read(&self, text: &str) -> View;

    #[depends(return.data = text)]
    async fn read_async(&self, text: &str) -> View;
}

struct Provider;

impl Reader for Provider {
    #[depends(return.data = text)]
    fn read(&self, text: &str) -> View {
        View { data: text }
    }

    #[depends(return.data = text)]
    async fn read_async(&self, text: &str) -> View {
        View { data: text }
    }
}

trait DefaultReader {
    #[depends(return.data = text)]
    fn read(&self, text: &str) -> View {
        View { data: text }
    }
}

impl DefaultReader for Provider {}

#[test]
fn trait_method_contracts_follow_borrowed_fields() {
    let value = String::from("value");
    let provider = Provider;
    let result = Reader::read(&provider, &value);
    assert_eq!(result.data, "value");
}

#[test]
fn async_trait_method_contracts_expand() {
    let value = String::from("value");
    let provider = Provider;
    let _future = provider.read_async(&value);
}

#[test]
fn default_trait_method_contracts_expand() {
    let value = String::from("value");
    let provider = Provider;
    let result = DefaultReader::read(&provider, &value);
    assert_eq!(result.data, "value");
}
