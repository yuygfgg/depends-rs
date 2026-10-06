# depends-rs

`depends-rs` lets you express Rust borrow relationships declaratively by mapping data dependencies instead of manually wiring lifetime generics.

`depends-rs` replaces manual lifetime wiring with clear data-flow contracts. The procedural macros generate ordinary Rust lifetime parameters and `where` clauses, while `rustc` continues to rigorously check every borrow, function body, and outlives rule.

`depends-rs` can be seen as a spiritual successor to [Cyan](https://github.com/yuygfgg/cyan).

---

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

The generated Rust code expands to:

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

This function returns a struct holding two independent borrows:
- `return.left` is tied to the lifetime of `left`.
- `return.right` is tied to the lifetime of `right`.
- Both inputs must remain valid while the returned `Pair` is in use, but each input borrow can be released independently.

---

## Core Mental Model

`depends-rs` separates borrow architecture into two declarative steps:

1. **Lifetime Shapes (`#[lifetimes]`)**: Attached to `struct`, `enum`, or `impl` items. It inspects your data layout and automatically introduces independent lifetime parameters for reference fields, naming them after the fields (e.g. `'head`, `'tail`, `'parts_head`).
2. **Lifetime Contracts (`#[depends]`)**: Attached to functions and methods. Instead of manually inventing generic lifetime variables, you declare how outputs depend on inputs through intuitive path expressions (e.g. `return.field = input.field` or `out.data <= cfg.name`).

---

## Syntax Cheat Sheet

### Contract Operators

| Operator | Target | Source | Semantic | Generated Rust |
|:---|:---|:---|:---|:---|
| `=` | Leaf or Aggregate | Leaf | **Exact assignment / Broadcast**: Binds target leaf to source lifetime. If target is an aggregate, broadcasts to all unmatched leaves. | Shared lifetime parameter (`'src`) |
| `<=` | Leaf | Leaf | **Outlives constraint**: Enforces that target must live *no longer than* source (target is bounded by source). | `where 'source: 'target` |
| `~=` | Aggregate | Aggregate | **Structural mapping**: Recursively matches and binds all lifetime leaves between two matching aggregate shapes 1-to-1. | Matching shared lifetime parameters |
| `~<=` | Aggregate | Aggregate | **Structural outlives**: Recursively applies outlives constraints to all corresponding leaves between two matching aggregates. | `where 'source_i: 'target_i` |

> [!TIP]
> **Outlives Intuition**: Rust writes `'source: 'target` to mean "source outlives target" (source lives at least as long as target). In `depends-rs`, `Target <= Source` means **"Target's lifetime is bounded within Source's lifetime"** (Target $\le$ Source).

### Path Expressions

| Syntax | Example | Description |
|:---|:---|:---|
| `return` | `return = text` | The return value root |
| Parameter name | `text`, `cfg` | An input parameter root |
| Named field | `cfg.name`, `return.parts.tail` | Struct field navigation |
| Tuple index | `tuple.0`, `tuple.1` | Tuple element index |
| Enum variant | `return.Right.value`, `return.Pair.1` | Enum variant field or tuple element |
| Container index | `entries.0`, `entries.1` | Multi-argument generic container type arguments (e.g. `HashMap<K, V>`) |
| Method receiver | `self.data` | Field stored inside `&self` or `self` |
| Nested reference | `input.deref` | Inner reference in nested references like `&mut &mut T` |
| Explicit lifetime | `'a`, `'static` | Declared function lifetime or `'static` |

---

## Defining Borrowed Types with `#[lifetimes]`

### Structs and Nested Types

`#[lifetimes]` generates independent lifetime parameters for each reference field. Field names determine parameter names. When types are nested, paths are concatenated to retain full lifetime granularity:

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

### Generics and Standard Containers

- **Generic fields**: Dependency paths follow field names rather than generic parameter positions:
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

- **Multi-argument containers**: For containers with multiple type arguments (such as `HashMap<K, V>`), paths use numeric indices (`.0` for key, `.1` for value) on the field (e.g. `entries.0`, `entries.1`).

---

### Impl Blocks and Methods

When implementing methods on types decorated with `#[lifetimes]`, attach `#[lifetimes]` to the `impl` block to automatically reconstruct the type's lifetime parameters:

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

#### Disentangling `&self` from Stored Fields

In ordinary Rust, `fn data(&self) -> &str` triggers lifetime elision, binding the return lifetime to the temporary `&self` borrow. Callers cannot release or mutate `View` while holding that return reference.

With `#[depends]`, you can bind the return value directly to the stored lifetime in `self.data`, completely independent of the receiver borrow:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[lifetimes]
impl View {
    #[depends(return = self.data)]
    fn data(&self) -> &str { self.data }

    #[depends(return = self.data)]
    fn into_data(self) -> &str { self.data }
}
```

The enclosing self shape also applies to every bare `Self` occurrence. For example, an argument `other: &Self` exposes `other.data`, and a `Self` return type exposes `return.data` to dependency contracts.

---

## Declaring Contracts with `#[depends]`

### Exact Field Mappings and Broadcasts (`=`)

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

When the target is an aggregate, `=` acts as a **broadcast relation**, assigning the source lifetime to all matching leaves. More specific relations override broader ones:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct Split {
    head: &str,
    tail: &str,
}

#[depends(return = text)]
fn duplicate(text: &str) -> Split {
    Split { head: text, tail: text }
}

#[lifetimes]
struct Outer {
    parts: Split,
    label: &str,
}

#[depends(
    return = text,
    return.parts.tail = focus
)]
fn split(text: &str, focus: &str) -> Outer {
    Outer {
        parts: Split { head: text, tail: focus },
        label: text,
    }
}
```

---

### Extracting Inner Fields from Inputs

When an input parameter is a borrowed struct, you can depend on either:
- The temporary borrow of the wrapper itself (`cfg`), or
- An inner borrowed field stored inside the wrapper (`cfg.name`):

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

### In-Place Updates and Outlives Constraints (`<=`)

When modifying an existing borrowed value through a mutable reference (`&mut View`), ordinary assignment (`=`) is invalid because a function cannot rebind the caller's pre-existing lifetime slot.

Instead, use `<=` to express an **outlives constraint** (`'source: 'target`): the incoming data must live at least as long as the slot being modified (`Target <= Source`):

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

The constraint `'name: 'data` ensures safety without forcing `out` to change its lifetime parameter.

---

### Structural Operations (`~=` and `~<=`)

When working with aggregate values by value or batch-updating aggregates, structural operators compare and map shapes automatically.

#### Structural Mappings (`~=`)

`~=` maps matching lifetime leaves 1-to-1 between two aggregate values. A by-value aggregate cannot be used as the source of `=`; use `~=` instead:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct State {
    name: &str,
    config: &str,
    cache: &str,
}

#[depends(return ~= old, return.cache = cache)]
fn replace_cache(old: State, cache: &str) -> State {
    State {
        name: old.name,
        config: old.config,
        cache,
    }
}
```

The `~=` relation maps `return.name` to `old.name` and `return.config` to `old.config`. The more specific `=` relation overrides the mapping for `return.cache`. The macro reports the first missing relative path if aggregate shapes do not match.

#### Structural Outlives (`~<=`)

`~<=` applies an outlives constraint across all matching leaves in two aggregate paths. Each source lifetime must outlive the corresponding target lifetime:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View {
    data: &str,
}

#[depends(out ~<= cfg)]
fn update(out: &mut View, cfg: &View) {
    out.data = cfg.data;
}
```

The relative paths below `out` and `cfg` must have the same shape. Use `<=` for single leaf constraints and `~<=` when the whole aggregate must be checked.

---

### Nested Mutable References (`.deref`)

Nested references have separate dependency paths. For `input: &mut &mut i32`, the outer borrow is named `input`, while the inner mutable reference is addressed as `input.deref`. When returning the inner reborrow, use `input.deref <= input` to specify that the outer borrow outlives the reborrow:

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

---

### Explicit Lifetime Sources and Targets

A declared function lifetime or `'static` can be used as the source or target of a relation:

```rust
use depends_rs::{depends, lifetimes};

#[lifetimes]
struct View { data: &str }

struct External<'a> { data: &'a str }

#[depends(return.data = 'a)]
fn extract<'a>(input: External<'a>) -> View {
    View { data: input.data }
}

#[depends(return.data = 'static)]
fn builtin() -> View {
    View { data: "builtin" }
}
```

A declared lifetime can also be the target of an outlives relation when interfacing with an external type that already contains an explicit lifetime parameter:

```rust
use depends_rs::depends;

struct External<'a> {
    data: &'a str,
}

#[depends('a <= text)]
fn make<'a>(text: &str) -> External<'a> {
    External { data: text }
}
```

The generated signature adds `'text: 'a` and preserves the external type's written lifetime argument.

---

### Async Functions

Contracts work seamlessly on `async fn`. The generated lifetime belongs to the function signature, ensuring the returned future resolves to a borrowed value valid for the contract's duration:

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

The input must stay alive until the future completes and the returned borrowed value is no longer used.

---

### Trait Methods

When a trait method returns a borrowed type, attach `#[depends]` to the method declaration. The same contract syntax works for signatures, default implementations, and `async fn`:

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

The trait itself does not need `#[lifetimes]`. Rustc verifies that every implementation conforms to the expanded signature.

---

### Consuming Transitions

If an in-place mutation cannot satisfy an existing lifetime slot (or you need to transition into a new lifetime), consume the old value by value and return a fresh one:

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

## Working with Opaque and External Types

`depends-rs` expands lifetime shapes by querying compile-time metadata generated by `#[lifetimes]`. For external types or unannotated local types, the macro cannot inspect their fields and treats them as **opaque boundaries**.

### Manual Opacity: `opaque(...)`

Use `opaque(...)` on `#[lifetimes]` or `#[depends]` to mark an unannotated type as a boundary. Pass the type path without generic arguments:

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

Here, `return.value.field` cannot refer to a field inside `ExternalType`. To expose its fields, annotate the type's definition with `#[lifetimes]` and remove the `opaque(...)` entry.

---

### Automatically Recognized Types (Built-in Opacity)

Lifetime-shape lookup is skipped automatically for standard and primitive types, treating them as built-in boundaries. The complete list of special-cased types is:

1. **Primitive types**:
   `str`, `bool`, `char`, `i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `f32`, and `f64`.
2. **Standard library paths**:
   Any path whose first segment is `std`, `core`, or `alloc`.
3. **Unqualified standard types**:
   The unqualified names `String`, `Vec`, `Option`, `Result`, `Box`, `Cow`, `Rc`, `Arc`, `Cell`, `RefCell`, `UnsafeCell`, `Pin`, `PhantomData`, `MaybeUninit`, `HashMap`, `HashSet`, `BTreeMap`, `BTreeSet`, `VecDeque`, `LinkedList`, `BinaryHeap`, `Mutex`, `RwLock`, `Path`, `PathBuf`, `OsStr`, `OsString`, `CStr`, and `CString`.
4. **Type parameters & associated types**:
   Type parameters and paths starting with a type parameter or `Self`, such as `T`, `T::Item`, and `Self::Item`.
5. **Explicit lifetime arguments**:
   Any path written with explicit lifetime arguments, such as `View<'a>`, `View<'static>`, or `View<'_>`, even if the type has `#[lifetimes]` metadata.

> [!NOTE]
> **Path Resolution**: These checks use the written syntax path without resolving type aliases or imports. A renamed standard-library type may need an explicit `opaque(...)` entry, while a custom local type named `Vec` will match the built-in list.

---

### Type Arguments on Opaque Containers

Opacity stops inspection of a type's internal struct fields, but it **does not** stop recursive inspection of enclosing references and generic type arguments on the final segment:
- For `rows: Vec<&str>`, it generates a lifetime parameter for `&str` accessible at `rows`.
- For `rows: Vec<View>`, an annotated `View` exposes `rows.data`.
- **Single type argument**: Uses the enclosing field path (e.g. `rows`).
- **Multiple type arguments**: Uses numeric indices corresponding to the type argument order:
  - For `entries: HashMap<&str, &str>`, the key lifetime is addressed as `entries.0`, and the value lifetime is addressed as `entries.1`.

---

### External Types with Explicit Lifetimes

If an external type declares its own lifetimes, wire them using ordinary Rust lifetime syntax:

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

Because `ExternalView<'a>` contains an explicit lifetime argument, metadata lookup is skipped automatically, so `opaque(ExternalView)` is unnecessary.

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

---

## How Rustc Enforces Soundness

`depends-rs` does not bypass Rust's borrow checker or introduce unsafe code:
- **No body inspection**: The macros only rewrite function signatures and item declarations.
- **Pure compiler checking**: `rustc` validates the function body, verifies that returned values match declared contracts, and ensures all outlives constraints hold.
- **Non-goals**: `depends-rs` intentionally avoids runtime checks, hidden typestates, Higher-Rank Trait Bounds (HRTB), Generic Associated Type (GAT) rewriting, compiler-internal type introspection, or inspecting unannotated external types.

---

## Verification

To run tests, checks, and formatting:

```text
cargo test --workspace --offline
cargo check --workspace --offline
cargo clippy --workspace --offline --all-targets -- -D warnings
./tests/compile_fail.sh
cargo fmt --all -- --check
```

---

## License

Licensed under the Mozilla Public License 2.0. See [LICENSE](LICENSE).
