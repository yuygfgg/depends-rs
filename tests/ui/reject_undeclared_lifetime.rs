use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[depends(return.data = 'missing)]
fn bad(text: &str) -> View {
    View { data: text }
}

fn main() {}
