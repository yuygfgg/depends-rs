# depends-rs

`depends-rs` lets you express Rust borrow relationships declaratively by mapping data dependencies instead of manually wiring lifetime generics.

It generates ordinary Rust. The Rust compiler still checks every borrow, every function body, and every outlives rule.

`depends-rs` can be seen as a spiritual successor to [Cyan](https://github.com/yuygfgg/cyan)

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

```rust
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

`#[lifetimes]` generates independent lifetime parameters for each reference field. Field names determine parameter names. Nested borrowed types retain full lifetime granularity through path prefixes:

```rust
use depends_rs::lifetimes;
#[lifetimes]
struct Split {
    head: &str,
    tail: &str,
}
// Expands to: struct Split<'head, 'tail> { head: &'head str, tail: &'tail str }

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

Paths follow the structural shape of your type definitions. Tuple elements use numeric indices (`.0`, `.1`), while enum variants use their variant names:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Tuple<T>(&str, T);
// Expands to: struct Tuple<'prefix, T>(&'prefix str, T);

#[lifetimes]
enum Selection<T> {
    Left { value: &str, extra: T },
    Right { value: &str },
    Pair(&str, &str),
}

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
  ```rust
  use depends_rs::{depends, lifetimes};

  #[lifetimes]
  struct View {
      data: &str,
  }

  #[lifetimes]
  struct Wrapper<T> {
      prefix: &str,
      value: T,
  }

  // To target `data` inside `value: View`: use `return.value.data`
  #[depends(return.prefix = text, return.value.data = data)]
  fn make(text: &str, data: &str) -> Wrapper<View> {
      Wrapper { prefix: text, value: View { data } }
  }
  ```

- **Containers (`Vec`, arrays, slices)**: Collections of references share a single lifetime parameter per field:
  ```rust
  use depends_rs::lifetimes;

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

```rust
use depends_rs::lifetimes;

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

`opaque(...)` marks a named type as a boundary in lifetime-shape expansion. The macro does not query metadata for that type or expose fields from its own definition as dependency paths. Annotate the type with `#[lifetimes]` when those fields must be addressable.

Use `opaque(...)` on `#[lifetimes]` or `#[depends]` for an unannotated type, including a type from another crate. Supply the type path as used in the item, without generic arguments:

```rust
use depends_rs::lifetimes;

struct ExternalType;

#[lifetimes(opaque(ExternalType))]
struct Holder {
    value: ExternalType,
    text: &str,
}
// Expands to: struct Holder<'text> { value: ExternalType, text: &'text str }
```

Here, `return.value.field` cannot refer to a field inside `ExternalType`. To expose such fields, annotate the type's definition with `#[lifetimes]` and remove its `opaque(...)` entry.

Lifetime-shape lookup is also skipped automatically for:

- primitive names: `str`, `bool`, `char`, `i8` through `i128`, `u8` through `u128`, `isize`, `usize`, `f32`, and `f64`;
- paths whose first segment is `std`, `core`, or `alloc`;
- the unqualified names `String`, `Vec`, `Option`, `Result`, `Box`, `Cow`, `Rc`, `Arc`, `Cell`, `RefCell`, `UnsafeCell`, `Pin`, `PhantomData`, `MaybeUninit`, `HashMap`, `HashSet`, `BTreeMap`, `BTreeSet`, `VecDeque`, `LinkedList`, `BinaryHeap`, `Mutex`, `RwLock`, `Path`, `PathBuf`, `OsStr`, `OsString`, `CStr`, and `CString`;
- type parameters and paths that start with a type parameter or `Self`, such as `T`, `T::Item`, and `Self::Item`;
- paths with any explicit lifetime argument, including `View<'a>`, `View<'static>`, and `View<'_>`, even if the type has `#[lifetimes]` metadata.

These checks use the written path, without resolving imports or aliases. A renamed standard-library type may need an `opaque(...)` entry. A local type named `Vec` also matches the built-in list.

If an opaque type has its own lifetime parameters, write the required parameters, arguments, and relationships with ordinary Rust syntax:

```rust
use depends_rs::depends;

struct ExternalView<'a> {
    data: &'a str,
}

#[depends]
fn external_view<'a>(text: &'a str) -> ExternalView<'a> {
    ExternalView { data: text }
}
```

The shared `'a` connects the input borrow to the returned value. The explicit argument in `ExternalView<'a>` already skips lookup, so `opaque(ExternalView)` is unnecessary here. A contract such as `return.data = text` cannot inspect this occurrence. Rust still checks the declared lifetimes and the function body.

Opacity does not stop all recursive processing. The macro still processes an enclosing reference and type arguments on the final path segment. For `rows: Vec<&str>`, it generates a lifetime for `&str`, accessible at `rows`. For `rows: Vec<View>`, an annotated `View` can expose `rows.data`. A single type argument uses the enclosing dependency path; multiple type arguments use numeric indices, such as `entries.0` and `entries.1` for `HashMap<&str, &str>`. These paths come from the written type arguments, without inspection of the container's fields.

---

## Declaring Contracts with `#[depends]`

### Return Mappings (`=`)

The basic building block maps an input borrow to a returned field:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

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

<details>
<summary><b>See generated signature</b></summary>

```rust
struct View<'text> {
    data: &'text str,
}

async fn make<'text>(text: &'text str) -> View<'text> {
    View { data: text }
}
```
</details>

The input must stay alive until the future finishes and the returned borrowed value is no longer used.

---

### Trait Methods

When a trait method returns a borrowed type, add `#[depends]` to the method. The same syntax works for method declarations, default methods, and async methods:

```rust
use depends_rs::{depends, lifetimes};

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

// Implementations can use the same contract.
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

<details>
<summary><b>See generated signatures</b></summary>

```rust
struct View<'text> {
    data: &'text str,
}

trait Reader {
    fn read<'text>(&self, text: &'text str) -> View<'text>;
    async fn read_async<'text>(&self, text: &'text str) -> View<'text>;
}
```
</details>

The trait itself does not need `#[lifetimes]`. Rustc checks that each implementation satisfies the expanded signature.

---

### Extracting Inner Fields from Inputs

When an input parameter is already a borrowed struct, you can depend on either:
- The temporary borrow of the wrapper itself (`cfg`), or
- An inner borrowed field inside the wrapper (`cfg.name`):

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Config {
    name: &str,
}

#[depends(return = cfg.name)]
fn get(cfg: &Config) -> &str {
    cfg.name
}
```

<details>
<summary><b>See generated signature</b></summary>

```rust
struct Config<'name> {
    name: &'name str,
}

fn get<'config, 'name>(cfg: &'config Config<'name>) -> &'name str {
    cfg.name
}
```
</details>

Because the return value borrows `'name` rather than `'config`, callers can drop or release `cfg` while keeping the returned string reference valid.

---

### Broad Mappings and Specific Overrides

When an entire composite structure originates from a single source, use a broad mapping:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Split {
    head: &str,
    tail: &str,
}

// Maps all borrowed fields in `Split` to `text`
#[depends(return = text)]
fn duplicate(text: &str) -> Split {
    Split { head: text, tail: text }
}

#[lifetimes]
struct Outer {
    parts: Split,
    label: &str,
}

// More specific paths override a broad mapping.
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

```rust
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
```

<details>
<summary><b>See generated signature and where clause</b></summary>

```rust
struct View<'data> {
    data: &'data str,
}

struct Config<'name> {
    name: &'name str,
}

fn set_name<'slot, 'data, 'config, 'name>(
    out: &'slot mut View<'data>,
    cfg: &'config Config<'name>,
) where
    'name: 'data,
{
    out.data = cfg.name;
}
```
</details>

The constraint `'name: 'data` ensures safety without forcing `out` to change its lifetime type.

---

### Nested Mutable References

Nested mutable references have separate dependency paths. The outer borrow is named `input`; the inner mutable reference is addressed as `input.deref`. When the returned reference uses the inner slot, add an outlives relation for the reborrow:

```rust
use depends_rs::depends;

#[depends(return = input.deref, input.deref <= input)]
fn project(input: &mut &mut i32) -> &mut i32 {
    &mut **input
}
```

<details>
<summary><b>See generated signature and where clause</b></summary>

```rust
fn project<'outer, 'inner>(
    input: &'outer mut &'inner mut i32,
) -> &'inner mut i32
where
    'outer: 'inner,
{
    &mut **input
}
```
</details>

`input.deref <= input` states that the outer borrow must last at least as long as the inner returned borrow. Use the more specific path when a contract targets a nested reference instead of the outer borrow.

---

### Consuming Transitions

If a mutation cannot satisfy the old lifetime slot (or you need to rebind with a new lifetime), consume the old value by value and return a fresh one:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[depends(return.data = text)]
fn reset(_old: View, text: &str) -> View {
    View { data: text }
}
```

The caller drops ownership of the old view and receives a new view tied to `text`.

---

## Cross-Module & Crate Integration

`depends-rs` relies on macro-generated metadata tokens attached to annotated items. Standard Rust visibility, aliases, glob imports, and cross-crate re-exports work out of the box:

```rust
use depends_rs::depends;

mod model {
    use depends_rs::lifetimes;

    #[lifetimes]
    pub struct View {
        pub data: &str,
    }
}

// `model` can also be a dependency crate.
use model::View as PublicView;

#[depends(return.data = text)]
fn make(text: &str) -> PublicView {
    PublicView { data: text }
}
```

If `depends-rs` is renamed in your `Cargo.toml`, pass `crate_path`:

```rust
extern crate depends_rs as my_depends;

use my_depends::{depends, lifetimes};

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

Licensed under the Mozilla Public License 2.0. See [LICENSE](LICENSE).
