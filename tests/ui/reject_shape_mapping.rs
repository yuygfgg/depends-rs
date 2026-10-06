use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Source {
    left: &str,
}

#[lifetimes]
struct Target {
    right: &str,
}

#[depends(return ~= old)]
fn bad(old: Source) -> Target {
    Target { right: old.left }
}

fn main() {}
