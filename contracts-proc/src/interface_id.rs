//! Defines the `#[interface_id]` procedural macro.

use std::mem;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenTree};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute, FnArg, ItemTrait, LitStr, Result, Token,
    TraitItem,
};

struct MethodSelector {
    signature: String,
    rust_fn_name: String,
    selector_bytes: proc_macro2::TokenStream,
    selector: proc_macro2::TokenStream,
    cfg_attrs: Vec<Attribute>,
    dynamic: bool,
}

/// Computes an interface id as an associated function for the trait.
pub(crate) fn interface_id(
    _attr: &TokenStream,
    input: TokenStream,
) -> TokenStream {
    let mut input = parse_macro_input!(input as ItemTrait);
    let unsafety = input.unsafety;
    let selectors = match method_selectors(&mut input) {
        Ok(selectors) => selectors,
        Err(error) => return error.to_compile_error().into(),
    };
    let cfg_attrs = cfg_attrs(&input.attrs);
    let (module_selector_checks, method_selector_checks) =
        selector_collision_checks(&selectors, &cfg_attrs);

    let name = input.ident;
    let vis = input.vis;
    let attrs = input.attrs;
    let trait_items = input.items;
    let generics = input.generics.clone();
    let where_clause = &generics.where_clause;

    let supertrait_tokens = if input.supertraits.is_empty() {
        quote! {}
    } else {
        let supertraits = &input.supertraits;
        quote! { : #supertraits }
    };

    let selector_xors = selectors.iter().map(|selector| {
        let attrs = &selector.cfg_attrs;
        let value = &selector.selector;
        quote! {
            #(#attrs)*
            let __interface_id = __interface_id ^ #value;
        }
    });

    // Keep the same trait with an additional associated function
    // `interface_id`.
    quote! {
        #(#module_selector_checks)*
        #(#attrs)*
        #vis #unsafety trait #name #generics #supertrait_tokens #where_clause {
            #(#trait_items)*

            #[doc = concat!("Solidity interface id associated with ", stringify!(#name), " trait.")]
            #[doc = "Computed as a XOR of selectors for each function in the trait."]
            fn interface_id() -> alloy_primitives::aliases::B32
            where
                Self: Sized,
            {
                let __interface_id =
                    alloy_primitives::aliases::B32::new([0; 4]);
                #(#method_selector_checks)*
                #(#selector_xors)*
                __interface_id
            }
        }
    }
    .into()
}

fn method_selectors(input: &mut ItemTrait) -> Result<Vec<MethodSelector>> {
    let has_trait_generics = !input.generics.params.is_empty();
    let mut selectors = Vec::<MethodSelector>::new();

    for item in &mut input.items {
        let TraitItem::Fn(func) = item else {
            continue;
        };

        let cfg_attrs = cfg_attrs(&func.attrs);
        let override_fn_name = take_selector_name(&mut func.attrs)?;
        let rust_fn_name = func.sig.ident.to_string();
        let solidity_fn_name =
            override_fn_name.unwrap_or(rust_fn_name.to_case(Case::Camel));
        let arg_types = func.sig.inputs.iter().filter_map(|arg| match arg {
            FnArg::Typed(t) => Some(t.ty.clone()),
            FnArg::Receiver(_) => None,
        });
        let type_strings: Vec<String> =
            arg_types.clone().map(|ty| quote!(#ty).to_string()).collect();
        let uses_self_type = arg_types.clone().any(|ty| type_uses_self(&ty));
        let signature =
            format!("{}({})", solidity_fn_name, type_strings.join(","));
        let selector_bytes = quote! {
            stylus_sdk::function_selector!(#solidity_fn_name #(, #arg_types )*)
        };
        let method_selector = MethodSelector {
            signature: signature.clone(),
            rust_fn_name: rust_fn_name.clone(),
            selector: quote! {
                alloy_primitives::aliases::B32::new(#selector_bytes)
            },
            selector_bytes,
            cfg_attrs,
            dynamic: has_trait_generics || uses_self_type,
        };

        if let Some(existing) = selectors.iter().find(|existing| {
            existing.signature == signature
                && existing.cfg_attrs.is_empty()
                && method_selector.cfg_attrs.is_empty()
        }) {
            error!(@existing.rust_fn_name,
                "selector collision detected: function '{}' has the same selector as function '{}': {}",
                rust_fn_name,
                existing.rust_fn_name,
                signature,
            );
        }

        selectors.push(method_selector);
    }

    Ok(selectors)
}

fn cfg_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| {
            attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr")
        })
        .cloned()
        .collect()
}

fn take_selector_name(attrs: &mut Vec<Attribute>) -> Result<Option<String>> {
    let mut override_fn_name = None;
    for attr in mem::take(attrs) {
        if attr.path().is_ident("selector") {
            if override_fn_name.is_some() {
                error!(@attr.path(), "more than one selector attribute");
            }
            let args: SelectorArgs = match attr.parse_args() {
                Ok(args) => args,
                Err(error) => error!(@attr.path(), "{}", error),
            };
            override_fn_name = Some(args.name);
        } else {
            attrs.push(attr);
        }
    }
    Ok(override_fn_name)
}

fn selector_collision_checks(
    selectors: &[MethodSelector],
    cfg_attrs: &[Attribute],
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut module_checks = Vec::new();
    let mut method_checks = Vec::new();
    for (index, selector_a) in selectors.iter().enumerate() {
        for selector_b in &selectors[index + 1..] {
            let message = LitStr::new(
                &format!(
                    "selector collision detected: '{}' and '{}' have the same selector",
                    selector_a.signature, selector_b.signature
                ),
                proc_macro2::Span::call_site(),
            );
            let selector_bytes_a = &selector_a.selector_bytes;
            let selector_bytes_b = &selector_b.selector_bytes;
            let cfg_attrs_a = &selector_a.cfg_attrs;
            let cfg_attrs_b = &selector_b.cfg_attrs;
            let check = if selector_a.dynamic || selector_b.dynamic {
                quote! {
                    #(#cfg_attrs_a)*
                    #(#cfg_attrs_b)*
                    const {
                        assert!(
                            u32::from_be_bytes(#selector_bytes_a)
                                != u32::from_be_bytes(#selector_bytes_b),
                            #message,
                        );
                    };
                }
            } else {
                quote! {
                    #(#cfg_attrs)*
                    #(#cfg_attrs_a)*
                    #(#cfg_attrs_b)*
                    const _: () = {
                        assert!(
                            u32::from_be_bytes(#selector_bytes_a)
                                != u32::from_be_bytes(#selector_bytes_b),
                            #message,
                        );
                    };
                }
            };

            if selector_a.dynamic || selector_b.dynamic {
                method_checks.push(check);
            } else {
                module_checks.push(check);
            }
        }
    }

    (module_checks, method_checks)
}

/// Whether a selector argument type depends on the trait's `Self` type.
fn type_uses_self(ty: &syn::Type) -> bool {
    fn contains_self(tokens: proc_macro2::TokenStream) -> bool {
        tokens.into_iter().any(|token| match token {
            TokenTree::Ident(ident) => ident == "Self",
            TokenTree::Group(group) => contains_self(group.stream()),
            _ => false,
        })
    }

    contains_self(quote!(#ty))
}

/// Contains arguments of the `#[selector(..)]` attribute.
struct SelectorArgs {
    name: String,
}

impl Parse for SelectorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;

        if ident == "name" {
            let _: Token![=] = input.parse()?;
            let lit: LitStr = input.parse()?;
            Ok(SelectorArgs { name: lit.value() })
        } else {
            error!(@ident, "expected identifier 'name'")
        }
    }
}
