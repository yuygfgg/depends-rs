#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;

/// Add lifetime parameters to a struct, enum, or explicitly annotated impl.
#[proc_macro_attribute]
pub fn lifetimes(attr: TokenStream, item: TokenStream) -> TokenStream {
    depends_rs_core::expand_lifetimes(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

/// Elaborate dependency contracts into lifetime arguments and outlives bounds.
#[proc_macro_attribute]
pub fn depends(attr: TokenStream, item: TokenStream) -> TokenStream {
    depends_rs_core::expand_depends(attr.into(), item.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

/// Resume an expansion after Rust resolves a metadata macro.
#[doc(hidden)]
#[proc_macro]
pub fn __depends_continue(input: TokenStream) -> TokenStream {
    depends_rs_core::continue_expansion(input.into())
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}
