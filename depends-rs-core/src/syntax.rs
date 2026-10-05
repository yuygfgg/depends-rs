use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    ext::IdentExt,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Ident, Lifetime, LitInt, Path, Result, Token,
};

/// A path to one borrowed leaf, or to an aggregate of borrowed leaves.
#[derive(Clone, Debug)]
pub struct DependencyPath {
    pub segments: Vec<String>,
    pub span: Span,
}

impl PartialEq for DependencyPath {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments
    }
}
impl Eq for DependencyPath {}

impl DependencyPath {
    pub(crate) fn root(name: impl Into<String>, span: Span) -> Self {
        Self {
            segments: vec![name.into()],
            span,
        }
    }

    pub(crate) fn child(&self, name: impl Into<String>) -> Self {
        let mut result = self.clone();
        result.segments.push(name.into());
        result
    }

    pub(crate) fn append(&self, other: &[String]) -> Self {
        let mut result = self.clone();
        result.segments.extend_from_slice(other);
        result
    }

    pub(crate) fn starts_with(&self, prefix: &Self) -> bool {
        self.segments.starts_with(&prefix.segments)
    }

    pub(crate) fn display(&self) -> String {
        self.segments.join(".")
    }
}

impl Parse for DependencyPath {
    fn parse(input: ParseStream) -> Result<Self> {
        let first = Ident::parse_any(input)?;
        let mut path = Self::root(first.unraw().to_string(), first.span());
        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            if input.peek(LitInt) {
                let index: LitInt = input.parse()?;
                path.segments
                    .push(index.base10_parse::<usize>()?.to_string());
            } else {
                path.segments
                    .push(Ident::parse_any(input)?.unraw().to_string());
            }
        }
        Ok(path)
    }
}

/// One leaf and the lifetime parameter (or explicit static lifetime) it uses.
#[derive(Clone, Debug)]
pub struct LifetimeSlot {
    pub path: Vec<String>,
    pub lifetime: Lifetime,
}

/// A field whose internals are outside the metadata contract.
#[derive(Clone, Debug)]
pub struct OpaqueSlot {
    pub path: Vec<String>,
    pub ty: Path,
}

/// A field path occupied by a type parameter. The index follows type parameter
/// order, without counting lifetime or const parameters.
#[derive(Clone, Debug)]
pub struct GenericSlot {
    pub path: Vec<String>,
    pub parameter: usize,
}

/// Parameter order and field-to-parameter relationships for an opt-in type.
#[derive(Clone, Debug, Default)]
pub struct LifetimeShape {
    pub parameters: Vec<Lifetime>,
    pub slots: Vec<LifetimeSlot>,
    pub opaque: Vec<OpaqueSlot>,
    pub generic_slots: Vec<GenericSlot>,
}

/// A signature contract. Neither relation describes runtime provenance.
#[derive(Clone, Debug)]
pub enum Relation {
    Construct {
        target: DependencyPath,
        source: DependencyPath,
    },
    Outlives {
        target: DependencyPath,
        source: DependencyPath,
    },
}

impl Relation {
    pub(crate) fn target(&self) -> &DependencyPath {
        match self {
            Self::Construct { target, .. } | Self::Outlives { target, .. } => target,
        }
    }
    pub(crate) fn source(&self) -> &DependencyPath {
        match self {
            Self::Construct { source, .. } | Self::Outlives { source, .. } => source,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Options {
    pub opaque: Vec<Path>,
    pub crate_path: Path,
    pub relations: Vec<Relation>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            opaque: Vec::new(),
            crate_path: syn::parse_quote!(::depends_rs),
            relations: Vec::new(),
        }
    }
}

impl Parse for Options {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut options = Self::default();
        while !input.is_empty() {
            let look = input.fork();
            let first = Ident::parse_any(&look)?;
            if first == "opaque" && look.peek(syn::token::Paren) {
                input.parse::<Ident>()?;
                let contents;
                syn::parenthesized!(contents in input);
                options
                    .opaque
                    .extend(Punctuated::<Path, Token![,]>::parse_terminated(&contents)?);
            } else if first == "crate_path" && look.peek(Token![=]) {
                input.parse::<Ident>()?;
                input.parse::<Token![=]>()?;
                options.crate_path = input.parse()?;
            } else {
                let target = input.parse()?;
                let construct = if input.peek(Token![<=]) {
                    input.parse::<Token![<=]>()?;
                    false
                } else {
                    input.parse::<Token![=]>()?;
                    true
                };
                let source = input.parse()?;
                options.relations.push(if construct {
                    Relation::Construct { target, source }
                } else {
                    Relation::Outlives { target, source }
                });
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(options)
    }
}

pub(crate) fn path_key(path: &Path) -> String {
    path.to_token_stream().to_string()
}

pub(crate) fn bare_path(path: &Path) -> Path {
    let mut path = path.clone();
    for segment in &mut path.segments {
        segment.arguments = syn::PathArguments::None;
    }
    path
}

pub(crate) fn prepend_lifetimes(generics: &mut syn::Generics, lifetimes: &[Lifetime]) {
    let existing = std::mem::take(&mut generics.params);
    for lifetime in lifetimes {
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::parse_quote!(#lifetime)));
    }
    generics.params.extend(existing);
}

impl ToTokens for LifetimeShape {
    fn to_tokens(&self, out: &mut proc_macro2::TokenStream) {
        let params = &self.parameters;
        let slots = self.slots.iter().map(|slot| {
            let path = syn::LitStr::new(&slot.path.join("."), Span::call_site());
            let lifetime = &slot.lifetime;
            quote! { #path => #lifetime }
        });
        let opaque = self.opaque.iter().map(|region| {
            let path = syn::LitStr::new(&region.path.join("."), Span::call_site());
            let ty = &region.ty;
            quote!(#path => { #ty })
        });
        let generic_slots = self.generic_slots.iter().map(|slot| {
            let path = slot.path.join(".");
            let parameter = slot.parameter;
            quote!(#path => #parameter)
        });
        out.extend(quote! { [#(#params),*] [#(#slots),*] [#(#opaque),*] [#(#generic_slots),*] });
    }
}

impl Parse for LifetimeShape {
    fn parse(input: ParseStream) -> Result<Self> {
        let params;
        syn::bracketed!(params in input);
        let parameters: Vec<Lifetime> =
            Punctuated::<Lifetime, Token![,]>::parse_terminated(&params)?
                .into_iter()
                .collect();
        let contents;
        syn::bracketed!(contents in input);
        let mut slots = Vec::new();
        while !contents.is_empty() {
            let path: syn::LitStr = contents.parse()?;
            contents.parse::<Token![=>]>()?;
            let lifetime = contents.parse()?;
            slots.push(LifetimeSlot {
                path: path_segments(&path.value()),
                lifetime,
            });
            if !contents.is_empty() {
                contents.parse::<Token![,]>()?;
            }
        }
        let mut opaque = Vec::new();
        if input.peek(syn::token::Bracket) {
            let contents;
            syn::bracketed!(contents in input);
            while !contents.is_empty() {
                let path: syn::LitStr = contents.parse()?;
                contents.parse::<Token![=>]>()?;
                let group;
                syn::braced!(group in contents);
                opaque.push(OpaqueSlot {
                    path: path_segments(&path.value()),
                    ty: group.parse()?,
                });
                if !contents.is_empty() {
                    contents.parse::<Token![,]>()?;
                }
            }
        }
        let contents;
        syn::bracketed!(contents in input);
        let mut generic_slots = Vec::new();
        while !contents.is_empty() {
            let path: syn::LitStr = contents.parse()?;
            contents.parse::<Token![=>]>()?;
            let parameter: LitInt = contents.parse()?;
            generic_slots.push(GenericSlot {
                path: path_segments(&path.value()),
                parameter: parameter.base10_parse()?,
            });
            if !contents.is_empty() {
                contents.parse::<Token![,]>()?;
            }
        }
        let mut names = std::collections::BTreeSet::new();
        for parameter in &parameters {
            if !names.insert(parameter.ident.to_string()) {
                return Err(syn::Error::new(
                    parameter.span(),
                    "duplicate lifetime in shape metadata",
                ));
            }
        }
        for slot in &slots {
            if slot.lifetime.ident != "static" && !names.contains(&slot.lifetime.ident.to_string())
            {
                return Err(syn::Error::new(
                    slot.lifetime.span(),
                    "invalid lifetime metadata: a leaf refers to an undeclared parameter",
                ));
            }
        }
        Ok(Self {
            parameters,
            slots,
            opaque,
            generic_slots,
        })
    }
}

fn path_segments(path: &str) -> Vec<String> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

impl ToTokens for Options {
    fn to_tokens(&self, out: &mut proc_macro2::TokenStream) {
        let crate_path = &self.crate_path;
        let mut parts = vec![quote!(crate_path = #crate_path)];
        for path in &self.opaque {
            parts.push(quote! { opaque(#path) });
        }
        for relation in &self.relations {
            let (target, source, op) = match relation {
                Relation::Construct { target, source } => (target, source, quote!(=)),
                Relation::Outlives { target, source } => (target, source, quote!(<=)),
            };
            parts.push(quote! { #target #op #source });
        }
        out.extend(quote! { #(#parts),* });
    }
}

impl ToTokens for DependencyPath {
    fn to_tokens(&self, out: &mut proc_macro2::TokenStream) {
        let segments = self.segments.iter().map(|segment| {
            if segment.chars().all(|ch| ch.is_ascii_digit()) {
                LitInt::new(segment, self.span).into_token_stream()
            } else {
                Ident::new(segment, self.span).into_token_stream()
            }
        });
        out.extend(quote!(#(#segments).*));
    }
}
