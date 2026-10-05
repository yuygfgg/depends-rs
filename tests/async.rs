use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[depends(return.data = text)]
async fn make(text: &str) -> View {
    View { data: text }
}

fn run_ready<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the test future must be ready after one poll"),
    }
}

#[test]
fn async_function_contracts_follow_borrowed_fields() {
    let text = String::from("value");
    let view = run_ready(make(&text));
    assert_eq!(view.data, "value");
}
