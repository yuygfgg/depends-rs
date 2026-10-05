use depends_rs::{depends, lifetimes};
#[lifetimes]
struct View {
    data: &str,
}
#[lifetimes]
struct Config {
    name: &str,
}
#[depends(out.data <= cfg.name)]
fn set_name(out: &mut View, cfg: &Config) {
    out.data = cfg.name;
}
#[test]
fn ok() {
    let mut v = View { data: "" };
    let n = String::from("n");
    let c = Config { name: &n };
    set_name(&mut v, &c);
    assert_eq!(v.data, "n");
}
