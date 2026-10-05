use depends_rs::{depends, lifetimes};
#[lifetimes]
struct Config {
    name: &str,
}
#[allow(clippy::needless_lifetimes)]
#[depends(return = cfg.name)]
fn get(cfg: &Config) -> &str {
    cfg.name
}
#[test]
fn f() {
    let s = String::from("s");
    let c = Config { name: &s };
    assert_eq!(get(&c), "s");
}
