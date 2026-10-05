use depends_rs::lifetimes;
#[lifetimes]
enum E {
    One(&str),
    Two { x: &i32 },
}
#[test]
fn e() {
    let s = "x";
    let e = E::One(s);
    match e {
        E::One(v) => assert_eq!(v, "x"),
        _ => unreachable!(),
    }
    let e = E::Two { x: &1 };
    if let E::Two { x } = e {
        assert_eq!(*x, 1);
    }
}
