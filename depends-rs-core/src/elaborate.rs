use std::collections::{BTreeMap, BTreeSet};

use syn::{
    ext::IdentExt, spanned::Spanned, Error, GenericArgument, Generics, Lifetime, Path,
    PathArguments, Result, Type,
};

use crate::syntax::{bare_path, path_key, DependencyPath, GenericSlot, LifetimeShape, Options};

/// A signature or field occurrence of a lifetime. Shared explicit parameters
/// can appear at several paths. Each occurrence retains its semantic path.
#[derive(Clone)]
pub(crate) struct Binding {
    pub path: DependencyPath,
    pub lifetime: Lifetime,
}

#[derive(Clone)]
pub(crate) struct OpaqueRegion {
    pub path: DependencyPath,
    pub ty: Path,
}

/// One recursive type traversal serves all item kinds. An unresolved named
/// type suspends the expansion; the caller restarts with its returned shape.
pub(crate) struct Elaborator<'a> {
    pub bindings: Vec<Binding>,
    pub opaque_regions: Vec<OpaqueRegion>,
    pub generated: Vec<Lifetime>,
    pub pending: Vec<Path>,
    pub generic_slots: Vec<GenericSlot>,
    shapes: &'a BTreeMap<String, LifetimeShape>,
    options: &'a Options,
    type_parameters: BTreeMap<String, usize>,
    used_lifetimes: BTreeSet<String>,
    self_shape: Option<LifetimeShape>,
}

impl<'a> Elaborator<'a> {
    pub fn new(
        generics: &Generics,
        options: &'a Options,
        shapes: &'a BTreeMap<String, LifetimeShape>,
    ) -> Self {
        Self {
            bindings: Vec::new(),
            opaque_regions: Vec::new(),
            generated: Vec::new(),
            pending: Vec::new(),
            generic_slots: Vec::new(),
            shapes,
            options,
            type_parameters: generics
                .type_params()
                .enumerate()
                .map(|(index, p)| (p.ident.unraw().to_string(), index))
                .collect(),
            used_lifetimes: generics
                .lifetimes()
                .map(|p| p.lifetime.ident.to_string())
                .collect(),
            self_shape: None,
        }
    }

    pub fn fresh(&mut self, path: &DependencyPath) -> Lifetime {
        let readable = path
            .segments
            .iter()
            .flat_map(|segment| segment.split('_'))
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("_")
            .to_lowercase();
        let mut suffix = self.generated.len();
        loop {
            let name = format!("__depends_{readable}_{suffix}");
            if self.used_lifetimes.insert(name.clone()) {
                let lifetime = Lifetime::new(&format!("'{name}"), path.span);
                self.generated.push(lifetime.clone());
                return lifetime;
            }
            suffix += 1;
        }
    }

    /// Add the stored lifetime slots of an enclosing inherent impl. The
    /// receiver borrow itself is added from the method signature, so these
    /// slots remain distinct from `&self` or `&mut self`. Keep the shape so
    /// bare `Self` occurrences can reuse the same slots at their own paths.
    pub fn add_receiver_shape(&mut self, shape: &LifetimeShape) {
        self.self_shape = Some(shape.clone());
        self.used_lifetimes.extend(
            shape
                .parameters
                .iter()
                .map(|lifetime| lifetime.ident.to_string()),
        );
        self.bindings.extend(shape.slots.iter().map(|slot| Binding {
            path: DependencyPath {
                segments: slot.path.clone(),
                span: slot.lifetime.span(),
            },
            lifetime: slot.lifetime.clone(),
        }));
        self.opaque_regions
            .extend(shape.opaque.iter().map(|slot| OpaqueRegion {
                path: DependencyPath {
                    segments: slot.path.clone(),
                    span: slot.ty.span(),
                },
                ty: slot.ty.clone(),
            }));
    }

    pub fn has_lifetime(&self, lifetime: &Lifetime) -> bool {
        self.used_lifetimes.contains(&lifetime.ident.to_string())
    }

    pub fn ty(&mut self, ty: &mut Type, path: &DependencyPath) -> Result<()> {
        match ty {
            Type::Reference(reference) => {
                let lifetime = match &reference.lifetime {
                    Some(lifetime) if lifetime.ident != "_" => lifetime.clone(),
                    _ => self.fresh(path),
                };
                reference.lifetime = Some(lifetime.clone());
                self.bindings.push(Binding {
                    path: path.clone(),
                    lifetime,
                });
                // A reference to an aggregate exposes fields at the same root.
                // A reference to a reference needs a separate inner path.
                let inner_path = if matches!(*reference.elem, Type::Reference(_)) {
                    path.child("deref")
                } else {
                    path.clone()
                };
                self.ty(&mut reference.elem, &inner_path)?;
            }
            Type::Path(named) if named.qself.is_none() => {
                self.named(&mut named.path, path, false)?
            }
            Type::Tuple(tuple) => {
                for (index, element) in tuple.elems.iter_mut().enumerate() {
                    self.ty(element, &path.child(index.to_string()))?;
                }
            }
            Type::Array(array) => self.ty(&mut array.elem, path)?,
            Type::Slice(slice) => self.ty(&mut slice.elem, path)?,
            Type::Paren(paren) => self.ty(&mut paren.elem, path)?,
            Type::Group(group) => self.ty(&mut group.elem, path)?,
            Type::Ptr(pointer) => self.ty(&mut pointer.elem, path)?,
            // These forms retain normal Rust lifetime syntax and semantics.
            Type::BareFn(_)
            | Type::TraitObject(_)
            | Type::ImplTrait(_)
            | Type::Path(_)
            | Type::Never(_) => {}
            _ => {
                return Err(Error::new(
                    ty.span(),
                    "this type needs explicit Rust lifetime syntax",
                ))
            }
        }
        Ok(())
    }

    /// Elaborate an inherent-impl self type. A known local shape may retain
    /// explicit lifetime arguments such as `View<'a>` so method contracts can
    /// still address `self` fields without changing ordinary opaque-type
    /// handling elsewhere.
    pub fn receiver_ty(&mut self, ty: &mut Type, path: &DependencyPath) -> Result<()> {
        match ty {
            Type::Path(named) if named.qself.is_none() => self.named(&mut named.path, path, true),
            _ => self.ty(ty, path),
        }
    }

    fn named(
        &mut self,
        path: &mut Path,
        location: &DependencyPath,
        allow_explicit_shape: bool,
    ) -> Result<()> {
        let bare = bare_path(path);
        let key = path_key(&bare);
        if path.segments.len() == 1
            && path.leading_colon.is_none()
            && path.segments[0].ident == "Self"
        {
            if let Some(shape) = self.self_shape.clone() {
                for slot in &shape.slots {
                    let relative = slot
                        .path
                        .first()
                        .filter(|segment| segment.as_str() == "self")
                        .map(|_| &slot.path[1..])
                        .unwrap_or(&slot.path);
                    self.bindings.push(Binding {
                        path: location.append(relative),
                        lifetime: slot.lifetime.clone(),
                    });
                }
                for region in &shape.opaque {
                    let relative = region
                        .path
                        .first()
                        .filter(|segment| segment.as_str() == "self")
                        .map(|_| &region.path[1..])
                        .unwrap_or(&region.path);
                    self.opaque_regions.push(OpaqueRegion {
                        path: location.append(relative),
                        ty: region.ty.clone(),
                    });
                }
                return Ok(());
            }
        }
        if path.segments.len() == 1 && path.leading_colon.is_none() {
            if let Some(&parameter) = self
                .type_parameters
                .get(&path.segments[0].ident.unraw().to_string())
            {
                self.generic_slots.push(GenericSlot {
                    path: location.segments.clone(),
                    parameter,
                });
                return Ok(());
            }
        }
        let explicit = path
            .segments
            .iter()
            .any(|segment| match &segment.arguments {
                PathArguments::AngleBracketed(args) => args
                    .args
                    .iter()
                    .any(|a| matches!(a, GenericArgument::Lifetime(_))),
                _ => false,
            });
        let generic = path.segments.first().is_some_and(|segment| {
            self.type_parameters
                .contains_key(&segment.ident.unraw().to_string())
                || segment.ident == "Self"
        });
        // Opaque is a boundary around this named type. `named` still visits
        // its type arguments below, so `Vec<&str>` can expose the reference
        // while `Vec` itself does not need lifetime-shape metadata.
        let opaque = generic || (!allow_explicit_shape && explicit) || self.is_opaque(&bare);
        let shape = if opaque {
            None
        } else {
            self.shapes.get(&key).cloned()
        };
        if !opaque && shape.is_none() && !self.pending.iter().any(|p| path_key(p) == key) {
            self.pending.push(bare.clone());
        }
        if opaque && !is_primitive(&bare) && !generic {
            self.opaque_regions.push(OpaqueRegion {
                path: location.clone(),
                ty: bare,
            });
        }

        let mut inserted = Vec::new();
        if let Some(shape) = &shape {
            for region in &shape.opaque {
                self.opaque_regions.push(OpaqueRegion {
                    path: location.append(&region.path),
                    ty: region.ty.clone(),
                });
            }
            let explicit_lifetimes = path
                .segments
                .iter()
                .flat_map(|segment| match &segment.arguments {
                    PathArguments::AngleBracketed(args) => args
                        .args
                        .iter()
                        .filter_map(|argument| match argument {
                            GenericArgument::Lifetime(lifetime) => Some(lifetime.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect::<Vec<_>>();
            let mut arguments = BTreeMap::new();
            for parameter in &shape.parameters {
                let (lifetime, was_explicit) = if allow_explicit_shape {
                    if let Some(lifetime) = explicit_lifetimes.get(arguments.len()).cloned() {
                        (lifetime, true)
                    } else {
                        let first_path = shape
                            .slots
                            .iter()
                            .find(|slot| slot.lifetime == *parameter)
                            .map(|slot| location.append(&slot.path))
                            .unwrap_or_else(|| {
                                location.child(format!("lifetime{}", arguments.len()))
                            });
                        (self.fresh(&first_path), false)
                    }
                } else {
                    let first_path = shape
                        .slots
                        .iter()
                        .find(|slot| slot.lifetime == *parameter)
                        .map(|slot| location.append(&slot.path))
                        .unwrap_or_else(|| location.child(format!("lifetime{}", arguments.len())));
                    (self.fresh(&first_path), false)
                };
                arguments.insert(parameter.ident.to_string(), lifetime.clone());
                if !was_explicit {
                    inserted.push(GenericArgument::Lifetime(lifetime));
                }
            }
            for slot in &shape.slots {
                let lifetime = if slot.lifetime.ident == "static" {
                    slot.lifetime.clone()
                } else {
                    arguments.get(&slot.lifetime.ident.to_string()).cloned().ok_or_else(|| Error::new(path.span(), "invalid lifetime metadata: a leaf refers to an undeclared parameter"))?
                };
                self.bindings.push(Binding {
                    path: location.append(&slot.path),
                    lifetime,
                });
            }
        }

        let Some(last) = path.segments.last_mut() else {
            return Err(Error::new(
                path.span(),
                "a type path must contain a segment",
            ));
        };
        if let PathArguments::AngleBracketed(args) = &mut last.arguments {
            let count = args
                .args
                .iter()
                .filter(|arg| matches!(arg, GenericArgument::Type(_)))
                .count();
            let mut index = 0;
            for arg in &mut args.args {
                if let GenericArgument::Type(ty) = arg {
                    if let Some(shape) = &shape {
                        let locations = shape
                            .generic_slots
                            .iter()
                            .filter(|slot| slot.parameter == index)
                            .map(|slot| location.append(&slot.path))
                            .collect::<Vec<_>>();
                        self.type_argument(
                            ty,
                            &locations,
                            &location.child(format!("type{index}")),
                        )?;
                    } else {
                        let child = if count > 1 {
                            location.child(index.to_string())
                        } else {
                            location.clone()
                        };
                        self.ty(ty, &child)?;
                    }
                    index += 1;
                }
            }
        }
        if !inserted.is_empty() {
            let args = match &mut last.arguments {
                PathArguments::None => {
                    last.arguments = PathArguments::AngleBracketed(syn::parse_quote!(<>));
                    match &mut last.arguments {
                        PathArguments::AngleBracketed(args) => args,
                        _ => unreachable!(),
                    }
                }
                PathArguments::AngleBracketed(args) => args,
                _ => {
                    return Err(Error::new(
                        last.span(),
                        "use explicit Rust lifetimes for function trait arguments",
                    ))
                }
            };
            let original = std::mem::take(&mut args.args);
            args.args.extend(inserted);
            args.args.extend(original);
        }
        Ok(())
    }

    /// Elaborate each type argument once. All fields with that parameter use
    /// the same lifetime arguments, even when the parameter occurs repeatedly.
    fn type_argument(
        &mut self,
        ty: &mut Type,
        locations: &[DependencyPath],
        fallback: &DependencyPath,
    ) -> Result<()> {
        let origin = locations.first().unwrap_or(fallback);
        let binding_start = self.bindings.len();
        let opaque_start = self.opaque_regions.len();
        let generic_start = self.generic_slots.len();
        self.ty(ty, origin)?;
        let bindings = self.bindings.split_off(binding_start);
        let opaque_regions = self.opaque_regions.split_off(opaque_start);
        let generic_slots = self.generic_slots.split_off(generic_start);
        let prefix_len = origin.segments.len();
        for location in locations {
            self.bindings.extend(bindings.iter().map(|binding| Binding {
                path: location.append(&binding.path.segments[prefix_len..]),
                lifetime: binding.lifetime.clone(),
            }));
            self.opaque_regions
                .extend(opaque_regions.iter().map(|region| OpaqueRegion {
                    path: location.append(&region.path.segments[prefix_len..]),
                    ty: region.ty.clone(),
                }));
            self.generic_slots
                .extend(generic_slots.iter().map(|slot| GenericSlot {
                    path: location.append(&slot.path[prefix_len..]).segments,
                    parameter: slot.parameter,
                }));
        }
        Ok(())
    }

    /// Skip metadata lookup for explicit exclusions and built-in type names.
    /// This check uses path spelling, without resolving imports or aliases.
    /// `named` also skips lookup for explicit lifetime arguments and paths
    /// rooted in a type parameter or `Self`.
    fn is_opaque(&self, path: &Path) -> bool {
        if self
            .options
            .opaque
            .iter()
            .any(|p| path_key(p) == path_key(path))
        {
            return true;
        }
        if is_primitive(path) {
            return true;
        }
        let Some(first) = path.segments.first().map(|segment| &segment.ident) else {
            return true;
        };
        if first == "std" || first == "core" || first == "alloc" {
            return true;
        }
        path.segments.len() == 1
            && matches!(
                first.to_string().as_str(),
                "String"
                    | "Vec"
                    | "Option"
                    | "Result"
                    | "Box"
                    | "Cow"
                    | "Rc"
                    | "Arc"
                    | "Cell"
                    | "RefCell"
                    | "UnsafeCell"
                    | "Pin"
                    | "PhantomData"
                    | "MaybeUninit"
                    | "HashMap"
                    | "HashSet"
                    | "BTreeMap"
                    | "BTreeSet"
                    | "VecDeque"
                    | "LinkedList"
                    | "BinaryHeap"
                    | "Mutex"
                    | "RwLock"
                    | "Path"
                    | "PathBuf"
                    | "OsStr"
                    | "OsString"
                    | "CStr"
                    | "CString"
            )
    }
}

fn is_primitive(path: &Path) -> bool {
    path.segments.len() == 1
        && matches!(
            path.segments[0].ident.to_string().as_str(),
            "str"
                | "bool"
                | "char"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
        )
}

pub(crate) fn fields(
    elaborator: &mut Elaborator<'_>,
    fields: &mut syn::Fields,
    prefix: Option<&DependencyPath>,
) -> Result<()> {
    for (index, field) in fields.iter_mut().enumerate() {
        let name = field
            .ident
            .as_ref()
            .map(|i| i.unraw().to_string())
            .unwrap_or_else(|| index.to_string());
        let location = prefix
            .map(|p| p.child(name.clone()))
            .unwrap_or_else(|| DependencyPath::root(name, field.span()));
        elaborator.ty(&mut field.ty, &location)?;
    }
    Ok(())
}
