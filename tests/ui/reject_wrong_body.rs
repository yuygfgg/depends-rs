use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Pair { left: &str, right: &str }

#[depends(return.left = left, return.right = right)]
fn wrong(left: &str, right: &str) -> Pair {
    Pair { left: right, right }
}

fn main() {}
