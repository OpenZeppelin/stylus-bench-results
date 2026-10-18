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
    let selectors = match collect_selectors(&mut input) {
        Ok(selectors) => selectors,
        Err(error) => return error.to_compile_error().into(),
    };
    let unsafety = input.unsafety;

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

    let selector_checks = selectors.iter().enumerate().flat_map(
        |(left_index, (left_name, _, left_selector))| {
            selectors.iter().skip(left_index + 1).map(
                move |(right_name, _, right_selector)| {
                    let message = format!(
                        "selector collision detected between functions '{left_name}' and '{right_name}'"
                    );
                    quote! {
                        assert!(#left_selector != #right_selector, #message);
                    }
                },
            )
        },
    );
    let interface_id_selectors =
        selectors.iter().map(|(_, selector, _)| selector);

    // Keep the same trait with an additional associated function
    // `interface_id`.
    quote! {
        const _: () = {
            #(#selector_checks)*
        };

        #(#attrs)*
        #vis #unsafety trait #name #generics #supertrait_tokens #where_clause {
            #(#trait_items)*

            #[cfg(not(feature = "contract-client-gen"))]
            #[allow(missing_docs)]
            fn __stylus_trait_and_impl_must_be_tagged_with_public_macro(&self);

            #[doc = concat!("Solidity interface id associated with ", stringify!(#name), " trait.")]
            #[doc = "Computed as a XOR of selectors for each function in the trait."]
            fn interface_id() -> alloy_primitives::aliases::B32
            where
                Self: Sized,
            {
                #(#interface_id_selectors)^*
            }
        }
    }
    .into()
}

type Selector = (String, proc_macro2::TokenStream, proc_macro2::TokenStream);

fn collect_selectors(input: &mut ItemTrait) -> Result<Vec<Selector>> {
    let mut selectors_map = HashMap::<String, String>::new();
    let mut selectors = Vec::new();

    for item in &mut input.items {
        let TraitItem::Fn(func) = item else {
            continue;
        };

        let mut override_fn_name = None;
        for attr in mem::take(&mut func.attrs) {
            if attr.path().is_ident("selector") {
                if override_fn_name.is_some() {
                    return Err(syn::Error::new_spanned(
                        attr.path(),
                        "more than one selector attribute",
                    ));
                }
                let args: SelectorArgs = attr.parse_args()?;
                override_fn_name = Some(args.name);
            } else {
                // Put back any other attributes.
                func.attrs.push(attr);
            }
        }

        let rust_fn_name = func.sig.ident.to_string();
        let solidity_fn_name =
            override_fn_name.unwrap_or(rust_fn_name.to_case(Case::Camel));
        let arg_types: Vec<_> = func
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(typed) => Some(typed.ty.clone()),
                // Opt out any `self` arguments.
                FnArg::Receiver(_) => None,
            })
            .collect();

        let type_strings: Vec<String> =
            arg_types.iter().map(|ty| quote!(#ty).to_string()).collect();
        let signature =
            format!("{}({})", solidity_fn_name, type_strings.join(","));

        if let Some(existing_rust_fn_name) = selectors_map.get(&signature) {
            return Err(syn::Error::new_spanned(
                &func.sig.ident,
                format!(
                    "duplicate Solidity signature: function '{rust_fn_name}' has the same signature as function '{existing_rust_fn_name}': {signature}"
                ),
            ));
        }

        let selector_bytes = quote! {
            stylus_sdk::function_selector!(#solidity_fn_name #(, #arg_types )*)
        };
        let selector = quote! {
            alloy_primitives::aliases::B32::new(#selector_bytes)
        };
        let selector_value = quote! { u32::from_be_bytes(#selector_bytes) };

        selectors_map.insert(signature, rust_fn_name.clone());
        selectors.push((rust_fn_name, selector, selector_value));
    }

    Ok(selectors)
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
