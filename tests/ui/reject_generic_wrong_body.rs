use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View { data: &str }

#[lifetimes]
struct Wrapper<T> { value: T }

#[depends(return.value.data = a)]
fn bad(a: &str, b: &str) -> Wrapper<View> {
    Wrapper { value: View { data: b } }
}

fn main() {}
