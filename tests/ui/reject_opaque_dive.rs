use depends_rs::{depends, lifetimes};

struct External;

#[lifetimes(opaque(External))]
struct Holder {
    value: External,
}

#[depends(return.value.field = text)]
fn bad(text: &str) -> Holder {
    Holder { value: External }
}

fn main() {}
