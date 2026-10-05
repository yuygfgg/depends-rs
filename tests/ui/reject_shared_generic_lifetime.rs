use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View { data: &str }

#[lifetimes]
struct Repeated<T> { first: T, second: T }

#[depends(return.first.data = a, return.second.data = b)]
fn bad(a: &str, b: &str) -> Repeated<View> {
    Repeated { first: View { data: a }, second: View { data: b } }
}

fn main() {}
