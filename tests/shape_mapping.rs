use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Config {
    value: &str,
}

#[lifetimes]
struct State {
    name: &str,
    config: Config,
    cache: &str,
    items: Vec<&str>,
    pair: (&str, &str),
}

#[lifetimes]
struct NameOnly {
    name: &str,
}

#[lifetimes]
struct Cached {
    name: &str,
    cache: &str,
}

#[lifetimes]
struct Two {
    left: &str,
    right: &str,
}

#[depends(return ~= old, return.cache = cache)]
fn replace_cache(old: State, cache: &str) -> State {
    State {
        name: old.name,
        config: old.config,
        cache,
        items: old.items,
        pair: old.pair,
    }
}

#[lifetimes]
struct Wrapper<T> {
    value: T,
    label: &str,
}

#[depends(return ~= old)]
fn identity(old: Wrapper<State>) -> Wrapper<State> {
    old
}

#[depends(return ~= old, return.cache = cache)]
fn add_cache(old: NameOnly, cache: &str) -> Cached {
    Cached {
        name: old.name,
        cache,
    }
}

#[depends(out ~<= cfg)]
fn update(out: &mut Two, cfg: &Two) {
    out.left = cfg.left;
    out.right = cfg.right;
}

#[depends(return ~<= old)]
fn preserve(old: Two) -> Two {
    old
}

#[test]
fn maps_nested_fields_and_overrides_one_leaf() {
    let old_name = String::from("old name");
    let old_config = String::from("old config");
    let new_cache = String::from("new cache");
    let result = replace_cache(
        State {
            name: &old_name,
            config: Config { value: &old_config },
            cache: "unused",
            items: vec!["old item"],
            pair: ("left", "right"),
        },
        &new_cache,
    );
    assert_eq!(
        (result.name, result.config.value, result.cache),
        ("old name", "old config", "new cache")
    );
    assert_eq!(
        (result.items[0], result.pair),
        ("old item", ("left", "right"))
    );
}

#[test]
fn a_specific_override_can_supply_a_new_leaf() {
    let name = String::from("name");
    let cache = String::from("cache");
    let result = add_cache(NameOnly { name: &name }, &cache);
    assert_eq!((result.name, result.cache), ("name", "cache"));
}

#[test]
fn mapping_preserves_lifetimes_through_generic_fields() {
    let label = String::from("label");
    let name = String::from("name");
    let config = String::from("config");
    let result;
    {
        let old = Wrapper {
            value: State {
                name: &name,
                config: Config { value: &config },
                cache: "cache",
                items: vec!["item"],
                pair: ("left", "right"),
            },
            label: &label,
        };
        result = identity(old);
    }
    assert_eq!(
        (result.value.name, result.value.config.value, result.label),
        ("name", "config", "label")
    );
}

#[test]
fn map_outlives_copies_matching_shape_constraints() {
    let left = String::from("left");
    let right = String::from("right");
    let mut out = Two {
        left: "old left",
        right: "old right",
    };
    let cfg = Two {
        left: &left,
        right: &right,
    };
    update(&mut out, &cfg);
    assert_eq!((out.left, out.right), ("left", "right"));
}

#[test]
fn map_outlives_can_cover_a_return_aggregate() {
    let left = String::from("left");
    let right = String::from("right");
    let result = preserve(Two {
        left: &left,
        right: &right,
    });
    assert_eq!((result.left, result.right), ("left", "right"));
}
