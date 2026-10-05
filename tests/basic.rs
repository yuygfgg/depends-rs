use depends_rs::{depends, lifetimes};

#[lifetimes]
pub struct Pair {
    pub left: &i32,
    pub right: &i32,
}

#[depends(return.left = a, return.right = b)]
fn pair(a: &i32, b: &i32) -> Pair {
    Pair { left: a, right: b }
}

#[test]
fn basic() {
    let a = 1;
    let b = 2;
    let x = pair(&a, &b);
    assert_eq!((*x.left, *x.right), (1, 2));
}
