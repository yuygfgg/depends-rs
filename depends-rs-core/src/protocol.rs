use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, Visibility};

use crate::syntax::LifetimeShape;

/// The metadata protocol version is defined once. All marker identifiers,
/// generated version lists, and diagnostics derive from this constant.
const PROTOCOL_VERSION: u8 = 1;

pub(crate) fn shape_request_marker() -> Ident {
    format_ident!("__depends_shape_v{PROTOCOL_VERSION}")
}

pub(crate) fn shape_result_marker() -> Ident {
    format_ident!("__depends_shape_result_v{PROTOCOL_VERSION}")
}

pub(crate) fn unsupported_protocol_message() -> String {
    format!("unsupported depends-rs metadata protocol; supported version: {PROTOCOL_VERSION}")
}

/// A deterministic helper name includes the input span to separate otherwise
/// identical public definitions in different modules. No registry is needed.
pub(crate) fn helper_ident(name: &Ident, item: &TokenStream) -> Ident {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in format!("{item} {:?}", name.span()).bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format_ident!("__depends_shape_{}_{hash:016x}", name, span = name.span())
}

pub(crate) fn metadata(
    name: &Ident,
    visibility: &Visibility,
    attrs: &[Attribute],
    item: &TokenStream,
    shape: &LifetimeShape,
) -> TokenStream {
    let helper = helper_ident(name, item);
    let request_marker = shape_request_marker();
    let result_marker = shape_result_marker();
    let version = PROTOCOL_VERSION;
    let unsupported_message = unsupported_protocol_message();
    let export = matches!(visibility, Visibility::Public(_)).then(|| quote!(#[macro_export]));
    let gating = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .collect::<Vec<_>>();
    quote! {
        #(#gating)*
        #[doc(hidden)]
        #[allow(unused_macros)]
        #export
        macro_rules! #helper {
            (@#request_marker callback = $callback:path; state = { $($state:tt)* };) => {
                $callback! { @#result_marker #shape { $($state)* } }
            };
            (@__depends_protocol_version callback = $callback:path; state = { $($state:tt)* };) => {
                $callback! { @__depends_protocol_versions [#version] { $($state)* } }
            };
            ($($request:tt)*) => {
                compile_error!(#unsupported_message);
            };
        }
        #(#gating)*
        #[doc(hidden)]
        #[allow(unused_imports)]
        #visibility use #helper as #name;
    }
}
