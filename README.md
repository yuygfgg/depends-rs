# depends-rs

`depends-rs` lets you express Rust borrow relationships declaratively by mapping data dependencies instead of manually wiring lifetime generics.

It generates ordinary Rust. The Rust compiler still checks every borrow, every function body, and every outlives rule.

## Quick Start

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Pair {
    left: &i32,
    right: &i32,
}

#[depends(return.left = left, return.right = right)]
fn pair(left: &i32, right: &i32) -> Pair {
    Pair { left, right }
}

fn main() {
    let left = 10;
    let right = 20;
    let result = pair(&left, &right);
    assert_eq!((*result.left, *result.right), (10, 20));
}
```

The generated form is:

```rust,ignore
struct Pair<'left, 'right> {
    left: &'left i32,
    right: &'right i32,
}

fn pair<'left, 'right>(
    left: &'left i32,
    right: &'right i32,
) -> Pair<'left, 'right> {
    Pair { left, right }
}
```

This function returns a small object that borrows two values. The first returned field stays tied to the first input. The second returned field stays tied to the second input. The inputs must remain alive while the returned object is used.

## Defining Borrowed Types with `#[lifetimes]`

### Structs and Nested Types

`#[lifetimes]` generates independent lifetime parameters for each reference field. Field names determine parameter names:

```rust,ignore
#[lifetimes]
struct Split {
    head: &str,
    tail: &str,
}
// Expands to: struct Split<'head, 'tail> { head: &'head str, tail: &'tail str }
```

Nested borrowed types retain full lifetime granularity through path prefixes:

```rust,ignore
#[lifetimes]
struct Outer {
    parts: Split,
    label: &str,
}
// Expands to: struct Outer<'parts_head, 'parts_tail, 'label> { ... }
```

Because lifetimes remain distinct, a function can return `parts.head` from one source, `parts.tail` from another, and `label` from a third.

---

### Tuples and Enums

Paths follow the structural shape of your type definitions:

```rust,ignore
#[lifetimes]
struct Tuple<T>(&str, T);
// Expands to: struct Tuple<'prefix, T>(&'prefix str, T);

#[lifetimes]
enum Selection<T> {
    Left { value: &str, extra: T },
    Right { value: &str },
    Pair(&str, &str),
}
```

Tuple elements use numeric indices (`.0`, `.1`), while enum variants use their variant names:

```rust,ignore
#[depends(
    return = a,
    return.Right.value = b,
    return.Pair.1 = b,
)]
fn select(a: &str, b: &str, index: u8) -> Selection<u8> {
    match index {
        0 => Selection::Left { value: a, extra: 7 },
        1 => Selection::Right { value: b },
        _ => Selection::Pair(a, b),
    }
}
```

---

### Generics and Containers

- **Generic fields**: Dependency paths follow field names, not generic argument indices:
  ```rust,ignore
  #[lifetimes]
  struct Wrapper<T> {
      prefix: &str,
      value: T,
  }

  // To target `data` inside `value: View`: use `return.value.data`
  #[depends(return.prefix = text, return.value.data = data)]
  fn make(text: &str, data: &str) -> Wrapper<View> { ... }
  ```

- **Containers (`Vec`, arrays, slices)**: Collections of references share a single lifetime parameter per field:
  ```rust,ignore
  #[lifetimes]
  struct Sample {
      rows: Vec<&str>,       // all rows share 'rows
      pair: (&str, &str),    // independent: 'pair_0, 'pair_1
      array: [&str; 2],      // all elements share 'array
  }
  ```

---

### Impl Blocks

When implementing methods on types decorated with `#[lifetimes]`, attach `#[lifetimes]` to the `impl` block to avoid manually writing lifetime generics:

```rust,ignore
#[lifetimes]
struct View {
    data: &str,
}

#[lifetimes]
impl View {
    fn data(&self) -> &str {
        self.data
    }
}
// Expands to: impl<'data> View<'data> { ... }
```

---

### Opaque and External Types

If a struct contains a foreign type that was not annotated with `#[lifetimes]`, use `opaque(...)` to inform the macro that the type should be treated as an opaque leaf without internal lifetime inspection:

```rust,ignore
struct ExternalType;

#[lifetimes(opaque(ExternalType))]
struct Holder {
    value: ExternalType,
    text: &str,
}
// Expands to: struct Holder<'text> { value: ExternalType, text: &'text str }
```

---

## Declaring Contracts with `#[depends]`

### Return Mappings (`=`)

The basic building block maps an input borrow to a returned field:

```rust,ignore
#[depends(return.data = text)]
fn make(text: &str) -> View {
    View { data: text }
}
// Lowered to: fn make<'text>(text: &'text str) -> View<'text>
```

---

### Async Functions

The same contract works on `async fn`. The generated lifetime belongs to the function signature, so the future resolves to a value that borrows from the input:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[depends(return.data = text)]
async fn make(text: &str) -> View {
    View { data: text }
}

async fn example() {
    let text = String::from("value");
    let view = make(&text).await;
    assert_eq!(view.data, "value");
}

fn main() {}
```

The generated signature is:

```rust,ignore
async fn make<'text>(text: &'text str) -> View<'text> {
    View { data: text }
}
```

The input must stay alive until the future finishes and the returned borrowed value is no longer used.

---

### Trait Methods

When a trait method returns a borrowed type, add `#[depends]` to the method. The same syntax works for method declarations, default methods, and async methods:

```rust,ignore
#[lifetimes]
struct View {
    data: &str,
}

trait Reader {
    #[depends(return.data = text)]
    fn read(&self, text: &str) -> View;

    #[depends(return.data = text)]
    async fn read_async(&self, text: &str) -> View;
}
```

The input and returned field use the same lifetime:

<details>
<summary><b>See generated signatures</b></summary>

```rust,ignore
trait Reader {
    fn read<'text>(&self, text: &'text str) -> View<'text>;
    async fn read_async<'text>(&self, text: &'text str) -> View<'text>;
}
```
</details>

Implementations can use the same contract:

```rust,ignore
struct Provider;

impl Reader for Provider {
    #[depends(return.data = text)]
    fn read(&self, text: &str) -> View {
        View { data: text }
    }

    #[depends(return.data = text)]
    async fn read_async(&self, text: &str) -> View {
        View { data: text }
    }
}
```

The trait itself does not need `#[lifetimes]`. Rustc checks that each implementation satisfies the expanded signature.

---

### Extracting Inner Fields from Inputs

When an input parameter is already a borrowed struct, you can depend on either:
- The temporary borrow of the wrapper itself (`cfg`), or
- An inner borrowed field inside the wrapper (`cfg.name`):

```rust,ignore
#[depends(return = cfg.name)]
fn get(cfg: &Config) -> &str {
    cfg.name
}
```

<details>
<summary><b>See generated signature</b></summary>

```rust,ignore
fn get<'config, 'name>(cfg: &'config Config<'name>) -> &'name str {
    cfg.name
}
```
Because the return value borrows `'name` rather than `'config`, callers can drop or release `cfg` while keeping the returned string reference valid.
</details>

---

### Broad Mappings and Specific Overrides

When an entire composite structure originates from a single source, use a broad mapping:

```rust,ignore
// Maps all borrowed fields in `Split` to `text`
#[depends(return = text)]
fn duplicate(text: &str) -> Split {
    Split { head: text, tail: text }
}
```

You can combine a broad mapping with fine-grained overrides. More specific paths take precedence:

```rust,ignore
#[depends(
    return = text,            // Default: all fields come from `text`
    return.parts.tail = focus // Override: only `tail` comes from `focus`
)]
fn split(text: &str, focus: &str) -> Outer {
    Outer {
        parts: Split { head: text, tail: focus },
        label: text,
    }
}
```

---

### In-Place Updates and Outlives Constraints (`<=`)

When modifying an existing borrowed value through a mutable reference (`&mut View`), ordinary assignment (`=`) is invalid because a function cannot rebind the caller's pre-existing lifetime slot.

Instead, use `<=` to express an **outlives constraint** (`'source: 'target`): the incoming data must live at least as long as the slot being modified.

```rust,ignore
#[depends(out.data <= cfg.name)]
fn set_name(out: &mut View, cfg: &Config) {
    out.data = cfg.name;
}
```

<details>
<summary><b>See generated signature and where clause</b></summary>

```rust,ignore
fn set_name<'slot, 'data, 'config, 'name>(
    out: &'slot mut View<'data>,
    cfg: &'config Config<'name>,
) where
    'name: 'data,
{
    out.data = cfg.name;
}
```
The constraint `'name: 'data` ensures safety without forcing `out` to change its lifetime type.
</details>

---

### Consuming Transitions

If a mutation cannot satisfy the old lifetime slot (or you need to rebind with a new lifetime), consume the old value by value and return a fresh one:

```rust,ignore
#[depends(return.data = text)]
fn reset(_old: View, text: &str) -> View {
    View { data: text }
}
```

The caller drops ownership of the old view and receives a new view tied to `text`.

---

## Cross-Module & Crate Integration

`depends-rs` relies on macro-generated metadata tokens attached to annotated items. Standard Rust visibility, aliases, glob imports, and cross-crate re-exports work out of the box:

```rust,ignore
use model::View as PublicView;

#[depends(return.data = text)]
fn make(text: &str) -> PublicView {
    PublicView { data: text }
}
```

If `depends-rs` is renamed in your `Cargo.toml`, pass `crate_path`:

```rust,ignore
#[lifetimes(crate_path = my_depends)]
struct View { data: &str }

#[depends(crate_path = my_depends, return.data = text)]
fn make(text: &str) -> View { View { data: text } }
```

## Rustc's role

The macro does not inspect the function body. It does not infer where a value came from at runtime. It only writes lifetime parameters, lifetime arguments, and outlives bounds.

`rustc` rejects a body that returns the wrong input borrow. It also rejects calls where an input is dropped too early. The macro generates no `unsafe` code and does not bypass Rust's ownership rules.

`depends-rs` does not add syntax for body inference, hidden typestate, HRTB, GAT rewriting, compiler type introspection, or arbitrary inspection of unannotated external types.

## Verification

```text
cargo test --workspace --offline
cargo check --workspace --offline
cargo clippy --workspace --offline --all-targets -- -D warnings
./tests/compile_fail.sh
cargo fmt --all -- --check
```

## License

MIT OR Apache-2.0
