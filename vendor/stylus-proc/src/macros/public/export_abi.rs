// Copyright 2024, Offchain Labs, Inc.
// For licensing, see https://github.com/OffchainLabs/stylus-sdk-rs/blob/main/licenses/COPYRIGHT.md

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, parse_str, visit_mut::VisitMut};

use super::types::{
    FnArgExtension, FnExtension, FnKind, InterfaceExtension, PublicImpl,
};
use crate::{consts::STRUCT_SUFFIX_FOR_TRAITS_IN_EXPORT_ABI, types::Purity};

#[derive(Debug)]
pub struct InterfaceAbi;

impl InterfaceExtension for InterfaceAbi {
    type FnExt = FnAbi;
    type Ast = syn::ItemImpl;

    fn build(_node: &syn::ItemImpl) -> Self {
        InterfaceAbi
    }

    fn codegen(iface: &PublicImpl<Self>) -> Self::Ast {
        let PublicImpl {
            generic_params,
            self_ty,
            where_clause,
            funcs,
            trait_,
            implements,
            associated_types,
            ..
        } = iface;

        let name = if trait_.is_some() {
            trait_.as_ref().unwrap().segments.last().unwrap().ident.to_string()
        } else {
            match self_ty {
                syn::Type::Path(path) => {
                    path.path.segments.last().unwrap().ident.clone().to_string()
                }
                _ => todo!(),
            }
        };

        let mut type_decls = TokenStream::new();
        for item in funcs {
            if let Some(ty) = &item.extension.output {
                let ty = replace_associated_types(ty, associated_types);
                let cfg_attrs = &item.cfg_attrs;
                type_decls.extend(quote! {
                    #(#cfg_attrs)*
                    for item in &<#ty as InnerTypes>::inner_types() {
                        if seen.insert(item.id) {
                            writeln!(f, "\n    {}", item.name)?;
                        }
                    }
                });
            }
        }
        let type_decls = quote! {
            let mut seen = HashSet::new();
            for item in ([] as [InnerType; 0]).iter() {
                if seen.insert(item.id) {
                    writeln!(f, "\n    {}", item.name)?;
                }
            }
            #type_decls
        };

        let mut abi = TokenStream::new();
        for func in funcs {
            if !matches!(func.kind, FnKind::Function) {
                continue;
            }

            let sol_name = func.sol_name.to_string();
            let sol_args = func.inputs.iter().enumerate().map(|(i, arg)| {
                let comma = if i > 0 { ", " } else { Default::default() };
                let name = arg.extension.pattern_ident.as_ref().map(ToString::to_string).unwrap_or_default();
                let ty = &arg.ty;
                quote! {
                    write!(f, "{}{}{}", #comma, <#ty as AbiType>::EXPORT_ABI_ARG, underscore_if_sol(#name))?;
                }
            });

            let sol_outs = if let Some(ty) = &func.extension.output {
                let ty = replace_associated_types(ty, associated_types);
                quote!(write_solidity_returns::<#ty>(f)?;)
            } else {
                quote!()
            };

            let sol_purity = match func.purity {
                Purity::Write => String::new(),
                x => format!(" {x}"),
            };
            let cfg_attrs = &func.cfg_attrs;

            abi.extend(quote! {
                #(#cfg_attrs)*
                {
                    write!(f, "\n    function {}(", #sol_name)?;
                    #(#sol_args)*
                    write!(f, ") external")?;
                    write!(f, #sol_purity)?;
                    #sol_outs
                    writeln!(f, ";")?;
                }
            });
        }

        let constructor_signature: Option<TokenStream> = funcs
            .iter()
            .filter_map(|func| match func.kind {
                FnKind::Constructor => {
                    let cfg_attrs = &func.cfg_attrs;
                    let sol_args = func.inputs.iter().enumerate().map(|(i, arg)| {
                        let comma = if i > 0 { ", " } else { Default::default() };
                        let name = arg.extension.pattern_ident.as_ref().map(ToString::to_string).unwrap_or_default();
                        let ty = &arg.ty;
                        quote! {
                            write!(f, "{}{}{}", #comma, <#ty as AbiType>::EXPORT_ABI_ARG, underscore_if_sol(#name))?;
                        }
                    });
                    let sol_purity = match func.purity {
                        Purity::Payable => " payable",
                        _ => "",
                    };
                    Some(quote! {
                        #(#cfg_attrs)*
                        {
                            use stylus_sdk::abi::AbiType;
                            use stylus_sdk::abi::export::underscore_if_sol;
                            write!(f, "constructor(")?;
                            #(#sol_args)*
                            write!(f, ")")?;
                            writeln!(f, #sol_purity)?;
                        }
                    })
                }
                _ => None,
            })
            .next();

        let struct_ty = if trait_.is_some() {
            let name =
                format!("{name}{STRUCT_SUFFIX_FOR_TRAITS_IN_EXPORT_ABI}");
            let ty: syn::Type = parse_str(&name)
                .expect("Failed to parse string into a syn::Type");
            ty
        } else {
            self_ty.clone()
        };

        let implements_names = implements.iter().map(|ty| {
            let name = match ty {
                syn::Type::Path(path) => {
                    path.path.segments.last().unwrap().ident.to_string()
                }
                _ => todo!(),
            };
            format!("I{name}")
        });
        let is_clause = if implements_names.len() > 0 {
            let names = implements_names.collect::<Vec<_>>().join(", ");
            format!(" is {names}")
        } else {
            String::new()
        };

        parse_quote! {
            impl<#generic_params> stylus_sdk::abi::GenerateAbi for #struct_ty where #where_clause {
                const NAME: &'static str = #name;

                fn fmt_abi(f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    use stylus_sdk::abi::{AbiType, GenerateAbi};
                    use stylus_sdk::abi::internal::write_solidity_returns;
                    use stylus_sdk::abi::export::{underscore_if_sol, internal::{InnerType, InnerTypes}};
                    use std::collections::HashSet;
                    write!(f, "interface I{}{}", #name, #is_clause)?;
                    write!(f, " {{")?;
                    #abi
                    #type_decls
                    writeln!(f, "}}")?;
                    Ok(())
                }

                fn fmt_constructor_signature(f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    #constructor_signature
                    Ok(())
                }
            }
        }
    }
}

fn replace_associated_types(
    ty: &syn::Type,
    associated_types: &[(syn::Ident, syn::Type)],
) -> syn::Type {
    struct AssociatedTypeReplacer<'a> {
        associated_types: &'a [(syn::Ident, syn::Type)],
        resolving: Vec<syn::Ident>,
    }

    impl syn::visit_mut::VisitMut for AssociatedTypeReplacer<'_> {
        fn visit_type_mut(&mut self, ty: &mut syn::Type) {
            if let syn::Type::Path(type_path) = ty {
                let ident = type_path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.clone());
                if let Some((ident, replacement)) = ident.and_then(|ident| {
                    self.associated_types
                        .iter()
                        .find(|(associated_type, _)| ident == *associated_type)
                        .map(|(associated_type, replacement)| {
                            (associated_type.clone(), replacement.clone())
                        })
                }) {
                    if !self.resolving.contains(&ident) {
                        self.resolving.push(ident);
                        let mut replacement = replacement;
                        self.visit_type_mut(&mut replacement);
                        self.resolving.pop();
                        *ty = replacement;
                        return;
                    }
                }
            }

            syn::visit_mut::visit_type_mut(self, ty);
        }
    }

    let mut resolved = ty.clone();
    AssociatedTypeReplacer { associated_types, resolving: Vec::new() }
        .visit_type_mut(&mut resolved);
    resolved
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::{parse_quote, Ident, Type};

    use super::replace_associated_types;

    #[test]
    fn replaces_associated_types_nested_in_result() {
        let associated_types: Vec<(Ident, Type)> = vec![
            (parse_quote!(Output), parse_quote!(Result<U256, Self::Error>)),
            (parse_quote!(Error), parse_quote!(ContractError)),
        ];

        let ty: Type = parse_quote!(Self::Output);
        let resolved = replace_associated_types(&ty, &associated_types);

        assert_eq!(
            quote!(#resolved).to_string(),
            quote!(Result<U256, ContractError>).to_string(),
        );
    }

    #[test]
    fn does_not_recurse_forever_for_same_named_concrete_error() {
        let associated_types: Vec<(Ident, Type)> =
            vec![(parse_quote!(Error), parse_quote!(Error))];

        let ty: Type = parse_quote!(Result<U256, Self::Error>);
        let resolved = replace_associated_types(&ty, &associated_types);

        assert_eq!(
            quote!(#resolved).to_string(),
            quote!(Result<U256, Error>).to_string(),
        );
    }
}

#[derive(Debug, Default)]
pub struct FnAbi {
    pub output: Option<syn::Type>,
}

impl FnExtension for FnAbi {
    type FnArgExt = FnArgAbi;

    fn build(node: &syn::ImplItemFn) -> Self {
        let output = match &node.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some(*ty.clone()),
        };
        FnAbi { output }
    }
}

#[derive(Debug)]
pub struct FnArgAbi {
    pub pattern_ident: Option<syn::Ident>,
}

impl FnArgExtension for FnArgAbi {
    fn build(node: &syn::FnArg) -> Self {
        let pattern_ident = if let syn::FnArg::Typed(pat_type) = node {
            pattern_ident(&pat_type.pat)
        } else {
            None
        };
        FnArgAbi { pattern_ident }
    }
}

/// finds the root type for a given arg
fn pattern_ident(pat: &syn::Pat) -> Option<syn::Ident> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.clone()),
        syn::Pat::Reference(pat_ref) => pattern_ident(&pat_ref.pat),
        _ => None,
    }
}
