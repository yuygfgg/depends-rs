#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for source in "$root"/tests/ui/reject_*.rs; do
    name=$(basename "$source" .rs)
    dir="$tmp/$name"
    mkdir -p "$dir/src"
    cat > "$dir/Cargo.toml" <<TOML
[package]
name = "$name"
version = "0.0.0"
edition = "2021"
[dependencies]
depends-rs = { path = "$root" }
TOML
    cp "$source" "$dir/src/main.rs"
    log="$dir/check.log"
    if cargo check --offline --manifest-path "$dir/Cargo.toml" >"$log" 2>&1; then
        echo "expected $name to fail" >&2
        exit 1
    fi
    case "$name" in
        reject_mutable_rebind)
            grep -F 'would rebind the lifetime of an existing value' "$log" >/dev/null
            ;;
        reject_wrong_body|reject_generic_wrong_body|reject_trait_wrong_body)
            grep -F 'lifetime may not live long enough' "$log" >/dev/null
            ;;
        reject_opaque_dive)
            grep -F 'cannot inspect lifetime shape' "$log" >/dev/null
            ;;
        reject_shared_generic_lifetime)
            grep -F 'conflicting dependencies for slots that share a lifetime' "$log" >/dev/null
            ;;
        reject_generic_argument_path)
            grep -F 'unknown dependency path `return.arg0.data`' "$log" >/dev/null
            ;;
    esac
done
