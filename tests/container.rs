use depends_rs::lifetimes;
#[lifetimes]
struct X {
    rows: Vec<&str>,
    pair: (&str, &str),
    arr: [&str; 2],
}
#[test]
fn c() {
    let a = "a";
    let b = "b";
    let x = X {
        rows: vec![a],
        pair: (a, b),
        arr: [a, b],
    };
    assert_eq!((x.rows[0], x.pair, x.arr), ("a", ("a", "b"), ["a", "b"]));
}
