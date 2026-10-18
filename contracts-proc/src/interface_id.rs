//! Defines the `#[interface_id]` procedural macro.

use std::{collections::HashMap, mem};

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, FnArg, ItemTrait, LitStr, Result, Token, TraitItem,
};

/// Computes an interface id as an associated function for the trait.
pub(crate) fn interface_id(
    _attr: &TokenStream,
    input: TokenStream,
) -> TokenStream {
    let mut input = parse_macro_input!(input as ItemTrait);

    let mut selectors_map = SelectorMap::new();
    let mut function_selectors = Vec::<FunctionSelector>::new();

    for item in &mut input.items {
        let TraitItem::Fn(func) = item else {
            continue;
        };

        let mut override_fn_name = None;
        for attr in mem::take(&mut func.attrs) {
            if attr.path().is_ident("selector") {
                if override_fn_name.is_some() {
                    error!(attr.path(), "more than one selector attribute");
                }
                let args: SelectorArgs = match attr.parse_args() {
                    Ok(args) => args,
                    Err(error) => error!(attr.path(), "{}", error),
                };
                override_fn_name = Some(args.name);
            } else {
                // Put back any other attributes.
                func.attrs.push(attr);
            }
        }

        let rust_fn_name = func.sig.ident.to_string();

        let solidity_fn_name =
            override_fn_name.unwrap_or(rust_fn_name.to_case(Case::Camel));

        let arg_types = func.sig.inputs.iter().filter_map(|arg| match arg {
            FnArg::Typed(t) => Some(t.ty.clone()),
            // Opt out any `self` arguments.
            FnArg::Receiver(_) => None,
        });

        // Build the function signature string for selector computation.
        let type_strings: Vec<String> =
            arg_types.clone().map(|ty| quote!(#ty).to_string()).collect();
        let signature =
            format!("{}({})", solidity_fn_name, type_strings.join(","));

        let selector = quote! {
            stylus_sdk::function_selector!(#solidity_fn_name #(, #arg_types )*)
        };

        // Store selector expression from every function in the trait.
        if let Some((existing_rust_fn_name, _)) = selectors_map.get(&signature)
        {
            error!(
                existing_rust_fn_name,
                "selector collision detected: function '{}' has the same selector as function '{}': {}",
                rust_fn_name,
                existing_rust_fn_name,
                signature,
            );
        }

        function_selectors.push((
            rust_fn_name.clone(),
            signature.clone(),
            selector.clone(),
        ));
        selectors_map.insert(
            signature,
            (
                rust_fn_name,
                quote! {
                    alloy_primitives::aliases::B32::new(#selector)
                },
            ),
        );
    }

    expand_interface(input, &selectors_map, &function_selectors)
}

type SelectorMap = HashMap<String, (String, proc_macro2::TokenStream)>;
type FunctionSelector = (String, String, proc_macro2::TokenStream);

fn expand_interface(
    input: ItemTrait,
    selectors_map: &SelectorMap,
    function_selectors: &[FunctionSelector],
) -> TokenStream {
    let unsafety = input.unsafety;
    let name = input.ident;
    let vis = input.vis;
    let attrs = input.attrs;
    let trait_items = input.items;
    let generics = input.generics.clone();
    let where_clause = &generics.where_clause;

    // Use module-level const assertions whenever selector types are concrete,
    // so collisions are diagnosed even if the generated interface_id method is
    // never called. Generic and associated types must instead be checked in
    // that method's const context, where the type parameters are in scope.
    let module_level_checks = generics.params.is_empty()
        && input.supertraits.is_empty()
        && !trait_items.iter().any(|item| matches!(item, TraitItem::Type(_)))
        && function_selectors
            .iter()
            .all(|(_, _, selector)| !selector.to_string().contains("Self"));

    // The proc macro cannot compute selectors itself: their concrete bytes are
    // produced by the SDK's const-evaluable `function_selector!` macro. Compare
    // every pair in const context so distinct signatures with identical 4-byte
    // selectors are rejected when the trait is compiled.
    let collision_assertions: Vec<_> = function_selectors
        .iter()
        .enumerate()
        .flat_map(|(index, (name_a, signature_a, selector_a))| {
            function_selectors[index + 1..]
                .iter()
                .map(move |(name_b, signature_b, selector_b)| {
                    let message = format!(
                        "Stylus interface_id: selector collision between `{name_a}` ({signature_a}) and `{name_b}` ({signature_b})"
                    );
                    quote! {
                        assert!(
                            u32::from_be_bytes(#selector_a)
                                != u32::from_be_bytes(#selector_b),
                            #message
                        );
                    }
                })
        })
        .collect();

    let module_collision_checks: Vec<_> = if module_level_checks {
        collision_assertions
            .iter()
            .map(|assertion| {
                quote! {
                    const _: () = { #assertion };
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let method_collision_checks = if module_level_checks {
        quote! {}
    } else {
        quote! {
            const {
                #(#collision_assertions)*
            }
        }
    };

    let supertrait_tokens = if input.supertraits.is_empty() {
        quote! {}
    } else {
        let supertraits = &input.supertraits;
        quote! { : #supertraits }
    };

    let selectors = selectors_map.values().map(|(_, tokens)| tokens);

    // Keep the same trait with an additional associated function
    // `interface_id`.
    quote! {
        #(#module_collision_checks)*

        #(#attrs)*
        #vis #unsafety trait #name #generics #supertrait_tokens #where_clause {
            #(#trait_items)*

            #[doc = concat!("Solidity interface id associated with ", stringify!(#name), " trait.")]
            #[doc = "Computed as a XOR of selectors for each function in the trait."]
            fn interface_id() -> alloy_primitives::aliases::B32
            where
                Self: Sized,
            {
                #method_collision_checks
                #(#selectors)^*
            }
        }
    }
    .into()
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
