use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View { data: &str }

#[lifetimes]
struct Wrapper<T> { value: T }

#[depends(return.arg0.data = text)]
fn bad(text: &str) -> Wrapper<View> {
    Wrapper { value: View { data: text } }
}

fn main() {}
