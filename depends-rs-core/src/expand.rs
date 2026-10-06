use std::collections::BTreeMap;

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    braced,
    ext::IdentExt,
    parse::{Parse, ParseStream},
    spanned::Spanned,
    visit_mut::VisitMut,
    Error, Ident, ImplItem, Item, ItemFn, ItemImpl, Lifetime, Path, Result, ReturnType, Signature,
    Token, TraitItemFn,
};

use crate::elaborate::{fields, Binding, Elaborator};
use crate::protocol::{self, metadata};
use crate::syntax::{path_key, DependencyPath, DependencySource, LifetimeShape, Options, Relation};

type Shapes = BTreeMap<String, LifetimeShape>;

/// An item accepted by `#[depends]`. A body-less trait method is not a Rust
/// `ItemFn`, so it needs a separate transport form while metadata is resolved.
#[derive(Clone)]
enum ExpansionItem {
    Rust(Item),
    TraitMethod(TraitItemFn),
}

pub fn expand_lifetimes(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let options: Options = syn::parse2(attr)?;
    if let Some(relation) = options.relations.first() {
        return Err(Error::new(
            relation.target_span(),
            "dependency relations belong on #[depends] functions",
        ));
    }
    let item: Item = syn::parse2(item)?;
    match &item {
        Item::Struct(_) | Item::Enum(_) => {}
        Item::Impl(item) if item.trait_.is_none() => {}
        _ => {
            return Err(Error::new(
                item.span(),
                "#[lifetimes] supports structs, enums, and inherent impl blocks",
            ))
        }
    }
    expand(options, ExpansionItem::Rust(item), Shapes::new())
}

pub fn expand_depends(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let options: Options = syn::parse2(attr)?;
    for relation in &options.relations {
        if let Relation::Construct { target, .. } | Relation::Map { target, .. } = relation {
            if !target
                .segments
                .first()
                .is_some_and(|segment| segment == "return")
            {
                let source = relation.source().display();
                let operator = if matches!(relation, Relation::Map { .. }) {
                    "~="
                } else {
                    "="
                };
                return Err(Error::new(target.span, format!(
                    "`{} {} {}` would rebind the lifetime of an existing value; use `{} <= {}`, or consume and return `{}`",
                    target.display(), operator, source, target.display(), source, target.segments[0]
                )));
            }
        }
    }
    let item = match syn::parse2::<ItemFn>(item.clone()) {
        Ok(item) => ExpansionItem::Rust(Item::Fn(item)),
        Err(_) => ExpansionItem::TraitMethod(syn::parse2::<TraitItemFn>(item)?),
    };
    expand(options, item, Shapes::new())
}

/// Each attempt starts from the original syntax. A suspended attempt discards
/// all edits and carries only resolved metadata into the next attempt.
fn expand(options: Options, original: ExpansionItem, shapes: Shapes) -> Result<TokenStream> {
    let mut item = original.clone();
    let generics = match &item {
        ExpansionItem::Rust(Item::Struct(item)) => item.generics.clone(),
        ExpansionItem::Rust(Item::Enum(item)) => item.generics.clone(),
        ExpansionItem::Rust(Item::Impl(item)) => item.generics.clone(),
        ExpansionItem::Rust(Item::Fn(item)) => item.sig.generics.clone(),
        ExpansionItem::TraitMethod(item) => item.sig.generics.clone(),
        ExpansionItem::Rust(item) => {
            return Err(Error::new(item.span(), "unsupported depends-rs item"))
        }
    };
    let mut elaborator = Elaborator::new(&generics, &options, &shapes);
    match &mut item {
        ExpansionItem::Rust(Item::Struct(item)) => fields(&mut elaborator, &mut item.fields, None)?,
        ExpansionItem::Rust(Item::Enum(item)) => {
            for variant in &mut item.variants {
                let prefix =
                    DependencyPath::root(variant.ident.unraw().to_string(), variant.span());
                fields(&mut elaborator, &mut variant.fields, Some(&prefix))?;
            }
        }
        ExpansionItem::Rust(Item::Impl(item)) => {
            let path = DependencyPath::root("self", item.self_ty.span());
            elaborator.receiver_ty(&mut item.self_ty, &path)?;
        }
        ExpansionItem::Rust(Item::Fn(item)) => {
            if let Some(receiver) = options.receiver.as_ref() {
                elaborator.add_receiver_shape(receiver);
            }
            elaborate_signature(&mut item.sig, &mut elaborator)?;
        }
        ExpansionItem::TraitMethod(item) => {
            if let Some(receiver) = options.receiver.as_ref() {
                elaborator.add_receiver_shape(receiver);
            }
            elaborate_signature(&mut item.sig, &mut elaborator)?;
        }
        ExpansionItem::Rust(_) => unreachable!(),
    }
    if !elaborator.pending.is_empty() {
        return PendingExpansion {
            item: original,
            options: options.clone(),
            shapes: shapes.clone(),
            pending: elaborator.pending,
            lookup: None,
        }
        .query();
    }

    let shape = LifetimeShape {
        generic_slots: elaborator.generic_slots.clone(),
        parameters: elaborator
            .generated
            .iter()
            .cloned()
            .chain(generics.lifetimes().map(|p| p.lifetime.clone()))
            .collect(),
        slots: elaborator
            .bindings
            .iter()
            .map(|binding| crate::syntax::LifetimeSlot {
                path: binding.path.segments.clone(),
                lifetime: binding.lifetime.clone(),
            })
            .collect(),
        opaque: elaborator
            .opaque_regions
            .iter()
            .map(|region| crate::syntax::OpaqueSlot {
                path: region.path.segments.clone(),
                ty: region.ty.clone(),
            })
            .collect(),
    };
    match &mut item {
        ExpansionItem::Rust(Item::Struct(item)) => {
            crate::syntax::prepend_lifetimes(&mut item.generics, &elaborator.generated);
            let transport = metadata(&item.ident, &item.vis, &item.attrs, &quote!(#item), &shape);
            Ok(quote!(#item #transport))
        }
        ExpansionItem::Rust(Item::Enum(item)) => {
            crate::syntax::prepend_lifetimes(&mut item.generics, &elaborator.generated);
            let transport = metadata(&item.ident, &item.vis, &item.attrs, &quote!(#item), &shape);
            Ok(quote!(#item #transport))
        }
        ExpansionItem::Rust(Item::Impl(item)) => {
            crate::syntax::prepend_lifetimes(&mut item.generics, &elaborator.generated);
            attach_receiver_context(item, &shape);
            Ok(quote!(#item))
        }
        ExpansionItem::Rust(Item::Fn(item)) => {
            let substitutions = apply_relations(&mut item.sig, &elaborator, &options.relations)?;
            let generated = elaborator
                .generated
                .into_iter()
                .filter(|lifetime| !substitutions.contains_key(&lifetime.ident.to_string()))
                .collect::<Vec<_>>();
            crate::syntax::prepend_lifetimes(&mut item.sig.generics, &generated);
            Ok(quote!(#item))
        }
        ExpansionItem::TraitMethod(item) => {
            let substitutions = apply_relations(&mut item.sig, &elaborator, &options.relations)?;
            let generated = elaborator
                .generated
                .into_iter()
                .filter(|lifetime| !substitutions.contains_key(&lifetime.ident.to_string()))
                .collect::<Vec<_>>();
            crate::syntax::prepend_lifetimes(&mut item.sig.generics, &generated);
            Ok(quote!(#item))
        }
        ExpansionItem::Rust(_) => unreachable!(),
    }
}

fn elaborate_signature(signature: &mut Signature, elaborator: &mut Elaborator<'_>) -> Result<()> {
    for (index, input) in signature.inputs.iter_mut().enumerate() {
        match input {
            syn::FnArg::Typed(argument) => {
                let name = match &*argument.pat {
                    syn::Pat::Ident(pattern) => pattern.ident.unraw().to_string(),
                    _ => format!("arg{index}"),
                };
                let path = DependencyPath::root(name, argument.pat.span());
                elaborator.ty(&mut argument.ty, &path)?;
            }
            syn::FnArg::Receiver(receiver) => {
                if let Some((_, lifetime)) = &mut receiver.reference {
                    let path = DependencyPath::root("self", receiver.self_token.span);
                    let lifetime = lifetime.get_or_insert_with(|| elaborator.fresh(&path));
                    elaborator.bindings.push(Binding {
                        path,
                        lifetime: lifetime.clone(),
                    });
                }
            }
        }
    }
    if let ReturnType::Type(_, ty) = &mut signature.output {
        let path = DependencyPath::root("return", ty.span());
        elaborator.ty(ty, &path)?;
    }
    Ok(())
}

fn attach_receiver_context(item: &mut ItemImpl, shape: &LifetimeShape) {
    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        for attribute in &mut method.attrs {
            if !attribute.path().is_ident("depends") {
                continue;
            }
            let receiver = quote! { receiver = { #shape } };
            match &mut attribute.meta {
                syn::Meta::List(list) if !list.tokens.is_empty() => {
                    let existing = list.tokens.clone();
                    list.tokens = quote! { #existing, #receiver };
                }
                syn::Meta::List(list) => list.tokens = receiver,
                _ => attribute.meta = syn::parse_quote!(depends(#receiver)),
            }
        }
    }
}

/// Return substitutions are simultaneous. Sequential text replacement could
/// accidentally replace a lifetime introduced by an earlier relation.
fn apply_relations(
    signature: &mut Signature,
    elaborator: &Elaborator<'_>,
    relations: &[Relation],
) -> Result<BTreeMap<String, Lifetime>> {
    let construct = relations
        .iter()
        .filter(|r| matches!(r, Relation::Construct { .. } | Relation::Map { .. }))
        .collect::<Vec<_>>();
    for (index, relation) in construct.iter().enumerate() {
        if construct[..index]
            .iter()
            .any(|prior| prior.target() == relation.target())
        {
            return Err(Error::new(
                relation.target().span,
                format!(
                    "duplicate dependency assignment `{}`",
                    relation.target().display()
                ),
            ));
        }
        if !elaborator
            .bindings
            .iter()
            .any(|binding| binding.path.starts_with(relation.target()))
        {
            return Err(path_error(elaborator, relation.target()));
        }
        match relation {
            Relation::Construct { source, .. } => {
                relation_source_lifetime(elaborator, source)?;
            }
            Relation::Map { source, target } => {
                validate_shape_mapping(elaborator, target, source, &construct, "~=", "=")?;
            }
            Relation::Outlives { .. }
            | Relation::LifetimeOutlives { .. }
            | Relation::MapOutlives { .. } => unreachable!(),
        }
    }
    let mut substitutions = BTreeMap::new();
    for binding in elaborator.bindings.iter().filter(|b| {
        b.path
            .segments
            .first()
            .is_some_and(|segment| segment == "return")
    }) {
        let Some(relation) = construct
            .iter()
            .filter(|r| binding.path.starts_with(r.target()))
            .max_by_key(|r| r.target().segments.len())
        else {
            continue;
        };
        let source_lifetime = match relation {
            Relation::Construct { source, .. } => relation_source_lifetime(elaborator, source)?,
            Relation::Map { source, target } => {
                mapped_lifetime(elaborator, binding, target, source)?
            }
            Relation::Outlives { .. }
            | Relation::LifetimeOutlives { .. }
            | Relation::MapOutlives { .. } => unreachable!(),
        };
        if binding.lifetime.ident == "static" && source_lifetime.ident != "static" {
            return Err(Error::new(
                relation.target().span,
                "an explicit 'static slot cannot be rebound to another dependency",
            ));
        }
        let key = binding.lifetime.ident.to_string();
        if let Some(previous) = substitutions.insert(key, source_lifetime.clone()) {
            if previous != source_lifetime {
                return Err(Error::new(
                    relation.target().span,
                    "conflicting dependencies for slots that share a lifetime",
                ));
            }
        }
    }
    if let ReturnType::Type(_, ty) = &mut signature.output {
        Substitute(&substitutions).visit_type_mut(ty);
    }
    for relation in relations.iter().filter(|r| {
        matches!(
            r,
            Relation::Outlives { .. }
                | Relation::LifetimeOutlives { .. }
                | Relation::MapOutlives { .. }
        )
    }) {
        match relation {
            Relation::Outlives { target, source } => {
                let source = relation_source_lifetime(elaborator, source)?;
                let target = unique_binding(elaborator, target)?;
                let target_lt = substitutions
                    .get(&target.lifetime.ident.to_string())
                    .cloned()
                    .unwrap_or_else(|| target.lifetime.clone());
                signature
                    .generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#source: #target_lt));
            }
            Relation::LifetimeOutlives { target, source } => {
                let source = relation_source_lifetime(elaborator, source)?;
                if target.ident == "_" {
                    return Err(Error::new(
                        target.span(),
                        "unsupported lifetime target `'_`; use a declared lifetime",
                    ));
                }
                if !elaborator.has_lifetime(target) && target.ident != "static" {
                    return Err(Error::new(
                        target.span(),
                        format!(
                            "undeclared lifetime target `{target}`; declare it on the function"
                        ),
                    ));
                }
                let target_lt = target.clone();
                signature
                    .generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#source: #target_lt));
            }
            Relation::MapOutlives { target, source } => {
                validate_shape_mapping(elaborator, target, source, &construct, "~<=", "<=")?;
                let DependencySource::Path(source) = source else {
                    unreachable!("the parser rejects lifetime sources for ~<=");
                };
                for binding in elaborator.bindings.iter().filter(|binding| {
                    binding.path.starts_with(target)
                        && binding.path.segments.len() > target.segments.len()
                }) {
                    let relative = &binding.path.segments[target.segments.len()..];
                    let source_path = source.append(relative);
                    let source_binding = unique_binding(elaborator, &source_path)?;
                    let target_lt = substitutions
                        .get(&binding.lifetime.ident.to_string())
                        .cloned()
                        .unwrap_or_else(|| binding.lifetime.clone());
                    let source_lt = source_binding.lifetime.clone();
                    signature
                        .generics
                        .make_where_clause()
                        .predicates
                        .push(syn::parse_quote!(#source_lt: #target_lt));
                }
            }
            _ => unreachable!(),
        }
    }
    // Only output occurrences are replaced. Keep explicit parameters and any
    // generated parameter that is also used by an input.
    substitutions.retain(|key, _| {
        !elaborator.bindings.iter().any(|b| {
            !b.path
                .segments
                .first()
                .is_some_and(|segment| segment == "return")
                && b.lifetime.ident == key
        })
    });
    Ok(substitutions)
}

struct Substitute<'a>(&'a BTreeMap<String, Lifetime>);
impl VisitMut for Substitute<'_> {
    fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
        if let Some(replacement) = self.0.get(&lifetime.ident.to_string()) {
            *lifetime = replacement.clone();
        }
    }
    fn visit_type_bare_fn_mut(&mut self, _: &mut syn::TypeBareFn) {}
    fn visit_type_trait_object_mut(&mut self, _: &mut syn::TypeTraitObject) {}
    fn visit_type_impl_trait_mut(&mut self, _: &mut syn::TypeImplTrait) {}
}
fn source_binding<'a>(
    elaborator: &'a Elaborator<'_>,
    path: &DependencyPath,
) -> Result<&'a Binding> {
    if path
        .segments
        .first()
        .is_some_and(|segment| segment == "return")
    {
        return Err(Error::new(
            path.span,
            "a dependency source must name an input borrow",
        ));
    }
    match unique_binding(elaborator, path) {
        Ok(binding) => Ok(binding),
        Err(_error)
            if elaborator
                .bindings
                .iter()
                .any(|binding| binding.path.starts_with(path) && binding.path != *path) =>
        {
            Err(Error::new(
                path.span,
                format!(
                    "aggregate dependency source `{}` needs the `~=` relation",
                    path.display()
                ),
            ))
        }
        Err(error) => Err(error),
    }
}

fn relation_source_lifetime(
    elaborator: &Elaborator<'_>,
    source: &DependencySource,
) -> Result<Lifetime> {
    match source {
        DependencySource::Path(path) => Ok(source_binding(elaborator, path)?.lifetime.clone()),
        DependencySource::Lifetime(lifetime) if lifetime.ident == "static" => {
            Ok(lifetime.clone())
        }
        DependencySource::Lifetime(lifetime) if lifetime.ident == "_" => Err(Error::new(
            lifetime.span(),
            "unsupported lifetime source `'_`; use a declared lifetime or `'static`",
        )),
        DependencySource::Lifetime(lifetime) if elaborator.has_lifetime(lifetime) => {
            Ok(lifetime.clone())
        }
        DependencySource::Lifetime(lifetime) => Err(Error::new(
            lifetime.span(),
            format!(
                "undeclared lifetime source `{lifetime}`; declare it on the function or use `'static`"
            ),
        )),
    }
}

fn validate_shape_mapping(
    elaborator: &Elaborator<'_>,
    target: &DependencyPath,
    source: &DependencySource,
    relations: &[&Relation],
    operator: &str,
    scalar_operator: &str,
) -> Result<()> {
    let DependencySource::Path(source) = source else {
        unreachable!("the parser rejects lifetime sources for ~=");
    };
    if source
        .segments
        .first()
        .is_some_and(|segment| segment == "return")
    {
        return Err(Error::new(
            source.span,
            "a dependency source must name an input aggregate",
        ));
    }
    let target_is_aggregate = elaborator.bindings.iter().any(|binding| {
        binding.path.starts_with(target) && binding.path.segments.len() > target.segments.len()
    });
    if !target_is_aggregate {
        return Err(Error::new(
            target.span,
            format!(
                "`{}` is not an aggregate target; use `{scalar_operator}` for a scalar relation",
                target.display(),
            ),
        ));
    }
    let source_is_aggregate = elaborator.bindings.iter().any(|binding| {
        binding.path.starts_with(source) && binding.path.segments.len() > source.segments.len()
    });
    if !source_is_aggregate {
        return Err(Error::new(
            source.span,
            format!(
                "`{}` is not an aggregate source for `{operator}`",
                source.display()
            ),
        ));
    }
    for binding in elaborator.bindings.iter().filter(|binding| {
        binding.path.starts_with(target) && binding.path.segments.len() > target.segments.len()
    }) {
        if relations.iter().any(|relation| {
            relation.target().segments.len() > target.segments.len()
                && binding.path.starts_with(relation.target())
        }) {
            continue;
        }
        let relative = &binding.path.segments[target.segments.len()..];
        let source_path = source.append(relative);
        if !elaborator
            .bindings
            .iter()
            .any(|candidate| candidate.path == source_path)
        {
            return Err(Error::new(
                binding.path.span,
                format!(
                    "shape mapping `{}` {operator} `{}` is missing source path `{}` for target `{}`",
                    target.display(),
                    source.display(),
                    source_path.display(),
                    binding.path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn mapped_lifetime(
    elaborator: &Elaborator<'_>,
    binding: &Binding,
    target: &DependencyPath,
    source: &DependencySource,
) -> Result<Lifetime> {
    let DependencySource::Path(source) = source else {
        unreachable!("the parser rejects lifetime sources for ~=");
    };
    let relative = &binding.path.segments[target.segments.len()..];
    let source_path = source.append(relative);
    unique_binding(elaborator, &source_path).map(|binding| binding.lifetime.clone())
}
fn unique_binding<'a>(
    elaborator: &'a Elaborator<'_>,
    path: &DependencyPath,
) -> Result<&'a Binding> {
    let mut matches = elaborator
        .bindings
        .iter()
        .filter(|binding| binding.path == *path);
    let binding = matches.next().ok_or_else(|| path_error(elaborator, path))?;
    if matches.any(|other| other.lifetime != binding.lifetime) {
        return Err(Error::new(
            path.span,
            format!(
                "ambiguous dependency path `{}`; use an explicit Rust lifetime",
                path.display()
            ),
        ));
    }
    Ok(binding)
}
fn path_error(elaborator: &Elaborator<'_>, path: &DependencyPath) -> Error {
    if let Some(region) = elaborator
        .opaque_regions
        .iter()
        .filter(|region| path.starts_with(&region.path))
        .max_by_key(|region| region.path.segments.len())
    {
        Error::new(path.span, format!("cannot inspect lifetime shape of `{}`; annotate its definition with #[lifetimes], or use explicit Rust lifetimes", region.ty.to_token_stream()))
    } else {
        Error::new(
            path.span,
            format!("unknown dependency path `{}`", path.display()),
        )
    }
}

/// Token transport has no process-global state and contains no file paths.
/// Shape keys are complete source paths, including aliases and qualifiers.
struct PendingExpansion {
    item: ExpansionItem,
    options: Options,
    pending: Vec<Path>,
    shapes: Shapes,
    lookup: Option<String>,
}
impl PendingExpansion {
    fn query(mut self) -> Result<TokenStream> {
        if self.pending.is_empty() {
            return expand(self.options, self.item, self.shapes);
        }
        let path = self.pending.remove(0);
        self.lookup = Some(path_key(&path));
        let callback = &self.options.crate_path;
        let request_marker = protocol::shape_request_marker();
        Ok(quote! {
            #path! {
                @#request_marker
                callback = #callback::__depends_continue;
                state = { #self };
            }
        })
    }
}
impl ToTokens for PendingExpansion {
    fn to_tokens(&self, output: &mut TokenStream) {
        let Self {
            item,
            options,
            pending,
            shapes,
            lookup,
        } = self;
        let entries = shapes
            .iter()
            .map(|(key, shape)| quote! { #key => { #shape } });
        let (kind, item) = match item {
            ExpansionItem::Rust(item) => (quote!(rust_item), quote!(#item)),
            ExpansionItem::TraitMethod(item) => (quote!(trait_method), quote!(#item)),
        };
        output.extend(quote! {
            item_kind = #kind;
            item = { #item };
            options = { #options };
            pending = [#(#pending),*];
            shapes = [#(#entries),*];
            lookup = #lookup;
        });
    }
}
impl Parse for PendingExpansion {
    fn parse(input: ParseStream) -> Result<Self> {
        fn field(input: ParseStream, expected: &str) -> Result<()> {
            let key: Ident = input.parse()?;
            if key != expected {
                return Err(Error::new(key.span(), "invalid depends-rs metadata state"));
            }
            input.parse::<Token![=]>()?;
            Ok(())
        }
        field(input, "item_kind")?;
        let kind: Ident = input.parse()?;
        input.parse::<Token![;]>()?;
        field(input, "item")?;
        let group;
        braced!(group in input);
        let item = match kind.to_string().as_str() {
            "rust_item" => ExpansionItem::Rust(group.parse::<Item>()?),
            "trait_method" => ExpansionItem::TraitMethod(group.parse::<TraitItemFn>()?),
            _ => {
                return Err(Error::new(
                    kind.span(),
                    "invalid depends-rs metadata item kind",
                ))
            }
        };
        input.parse::<Token![;]>()?;
        field(input, "options")?;
        let group;
        braced!(group in input);
        let options = group.parse()?;
        input.parse::<Token![;]>()?;
        field(input, "pending")?;
        let group;
        syn::bracketed!(group in input);
        let pending = syn::punctuated::Punctuated::<Path, Token![,]>::parse_terminated(&group)?
            .into_iter()
            .collect();
        input.parse::<Token![;]>()?;
        field(input, "shapes")?;
        let group;
        syn::bracketed!(group in input);
        let mut shapes = Shapes::new();
        while !group.is_empty() {
            let key: syn::LitStr = group.parse()?;
            group.parse::<Token![=>]>()?;
            let shape_group;
            braced!(shape_group in group);
            shapes.insert(key.value(), shape_group.parse()?);
            if !group.is_empty() {
                group.parse::<Token![,]>()?;
            }
        }
        input.parse::<Token![;]>()?;
        field(input, "lookup")?;
        let lookup: syn::LitStr = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(Self {
            item,
            options,
            pending,
            shapes,
            lookup: Some(lookup.value()),
        })
    }
}
struct ShapeResult {
    shape: LifetimeShape,
    state: PendingExpansion,
}
impl Parse for ShapeResult {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![@]>()?;
        let marker: Ident = input.parse()?;
        if marker != protocol::shape_result_marker() {
            return Err(Error::new(
                marker.span(),
                protocol::unsupported_protocol_message(),
            ));
        }
        let shape = input.parse()?;
        let group;
        braced!(group in input);
        Ok(Self {
            shape,
            state: group.parse()?,
        })
    }
}
pub fn continue_expansion(input: TokenStream) -> Result<TokenStream> {
    let ShapeResult { shape, mut state } = syn::parse2(input)?;
    let lookup = state
        .lookup
        .take()
        .ok_or_else(|| Error::new(Span::call_site(), "metadata result has no type path"))?;
    state.shapes.insert(lookup, shape);
    state.query()
}
