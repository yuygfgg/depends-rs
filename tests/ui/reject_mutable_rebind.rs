use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View { data: &str }

// This must fail. A mutable borrow updates an existing value in place.
#[depends(out.data = text)]
fn reset(out: &mut View, text: &str) {
    out.data = text;
}

fn main() {}
