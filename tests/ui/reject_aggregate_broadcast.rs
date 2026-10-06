use depends_rs::{depends, lifetimes};

#[lifetimes]
struct State {
    data: &str,
}

#[depends(return = old)]
fn bad(old: State) -> State {
    old
}

fn main() {}
