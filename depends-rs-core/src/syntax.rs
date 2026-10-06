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
        source: DependencySource,
    },
    Map {
        target: DependencyPath,
        source: DependencySource,
    },
    Outlives {
        target: DependencyPath,
        source: DependencySource,
    },
    LifetimeOutlives {
        target: Lifetime,
        source: DependencySource,
    },
    MapOutlives {
        target: DependencyPath,
        source: DependencySource,
    },
}

#[derive(Clone, Debug)]
pub enum DependencySource {
    Path(DependencyPath),
    Lifetime(Lifetime),
}

impl DependencySource {
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Path(path) => path.span,
            Self::Lifetime(lifetime) => lifetime.span(),
        }
    }

    pub(crate) fn display(&self) -> String {
        match self {
            Self::Path(path) => path.display(),
            Self::Lifetime(lifetime) => lifetime.to_string(),
        }
    }
}

impl Relation {
    pub(crate) fn target_span(&self) -> Span {
        match self {
            Self::Construct { target, .. }
            | Self::Map { target, .. }
            | Self::Outlives { target, .. }
            | Self::MapOutlives { target, .. } => target.span,
            Self::LifetimeOutlives { target, .. } => target.span(),
        }
    }

    pub(crate) fn target(&self) -> &DependencyPath {
        match self {
            Self::Construct { target, .. }
            | Self::Map { target, .. }
            | Self::Outlives { target, .. }
            | Self::MapOutlives { target, .. } => target,
            _ => panic!("lifetime relation target has no dependency path"),
        }
    }
    pub(crate) fn source(&self) -> &DependencySource {
        match self {
            Self::Construct { source, .. }
            | Self::Map { source, .. }
            | Self::Outlives { source, .. }
            | Self::LifetimeOutlives { source, .. }
            | Self::MapOutlives { source, .. } => source,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Options {
    pub opaque: Vec<Path>,
    pub crate_path: Path,
    pub relations: Vec<Relation>,
    pub receiver: Option<LifetimeShape>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            opaque: Vec::new(),
            crate_path: syn::parse_quote!(::depends_rs),
            relations: Vec::new(),
            receiver: None,
        }
    }
}

impl Parse for Options {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut options = Self::default();
        while !input.is_empty() {
            let look = input.fork();
            let first = if input.peek(Lifetime) {
                None
            } else {
                Some(Ident::parse_any(&look)?)
            };
            if first.as_ref().is_some_and(|first| first == "opaque") && look.peek(syn::token::Paren)
            {
                input.parse::<Ident>()?;
                let contents;
                syn::parenthesized!(contents in input);
                options
                    .opaque
                    .extend(Punctuated::<Path, Token![,]>::parse_terminated(&contents)?);
            } else if first.as_ref().is_some_and(|first| first == "crate_path")
                && look.peek(Token![=])
            {
                input.parse::<Ident>()?;
                input.parse::<Token![=]>()?;
                options.crate_path = input.parse()?;
            } else if first.as_ref().is_some_and(|first| first == "receiver")
                && look.peek(Token![=])
            {
                input.parse::<Ident>()?;
                input.parse::<Token![=]>()?;
                let group;
                syn::braced!(group in input);
                options.receiver = Some(group.parse()?);
            } else {
                let target = parse_target(input)?;
                let relation = if input.peek(Token![<=]) {
                    input.parse::<Token![<=]>()?;
                    let source = parse_source(input)?;
                    match target {
                        RelationTarget::Path(target) => Relation::Outlives { target, source },
                        RelationTarget::Lifetime(target) => {
                            Relation::LifetimeOutlives { target, source }
                        }
                    }
                } else if input.peek(Token![~]) {
                    input.parse::<Token![~]>()?;
                    if input.peek(Token![<=]) {
                        input.parse::<Token![<=]>()?;
                        let source = parse_source(input)?;
                        let target = match target {
                            RelationTarget::Path(target) => target,
                            RelationTarget::Lifetime(target) => {
                                return Err(syn::Error::new(
                                    target.span(),
                                    "the ~<= relation requires an aggregate dependency path",
                                ));
                            }
                        };
                        if matches!(source, DependencySource::Lifetime(_)) {
                            return Err(syn::Error::new(
                                source.span(),
                                "the ~<= relation requires an aggregate dependency path",
                            ));
                        }
                        Relation::MapOutlives { target, source }
                    } else {
                        input.parse::<Token![=]>()?;
                        let source = parse_source(input)?;
                        let target = match target {
                            RelationTarget::Path(target) => target,
                            RelationTarget::Lifetime(target) => {
                                return Err(syn::Error::new(
                                    target.span(),
                                    "the ~= relation requires an aggregate dependency path",
                                ));
                            }
                        };
                        if matches!(source, DependencySource::Lifetime(_)) {
                            return Err(syn::Error::new(
                                source.span(),
                                "the ~= relation requires an aggregate dependency path",
                            ));
                        }
                        Relation::Map { target, source }
                    }
                } else {
                    input.parse::<Token![=]>()?;
                    let target = match target {
                        RelationTarget::Path(target) => target,
                        RelationTarget::Lifetime(target) => {
                            return Err(syn::Error::new(
                                target.span(),
                                "the = relation requires a dependency path target",
                            ));
                        }
                    };
                    Relation::Construct {
                        target,
                        source: parse_source(input)?,
                    }
                };
                options.relations.push(relation);
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(options)
    }
}

fn parse_source(input: ParseStream) -> Result<DependencySource> {
    if input.peek(Lifetime) {
        Ok(DependencySource::Lifetime(input.parse()?))
    } else {
        Ok(DependencySource::Path(input.parse()?))
    }
}

enum RelationTarget {
    Path(DependencyPath),
    Lifetime(Lifetime),
}

fn parse_target(input: ParseStream) -> Result<RelationTarget> {
    if input.peek(Lifetime) {
        Ok(RelationTarget::Lifetime(input.parse()?))
    } else {
        Ok(RelationTarget::Path(input.parse()?))
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
        if let Some(receiver) = &self.receiver {
            parts.push(quote! { receiver = { #receiver } });
        }
        for path in &self.opaque {
            parts.push(quote! { opaque(#path) });
        }
        for relation in &self.relations {
            let relation = match relation {
                Relation::Construct { target, source } => quote!(#target = #source),
                Relation::Map { target, source } => quote!(#target ~= #source),
                Relation::Outlives { target, source } => quote!(#target <= #source),
                Relation::LifetimeOutlives { target, source } => quote!(#target <= #source),
                Relation::MapOutlives { target, source } => quote!(#target ~<= #source),
            };
            parts.push(relation);
        }
        out.extend(quote! { #(#parts),* });
    }
}

impl ToTokens for DependencySource {
    fn to_tokens(&self, out: &mut proc_macro2::TokenStream) {
        match self {
            Self::Path(path) => path.to_tokens(out),
            Self::Lifetime(lifetime) => lifetime.to_tokens(out),
        }
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
