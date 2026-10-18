use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg};

pub(crate) fn vm_test(_attr: &TokenStream, input: TokenStream) -> TokenStream {
    let item_fn = parse_macro_input!(input as syn::ItemFn);
    let attrs = &item_fn.attrs;
    let sig = &item_fn.sig;
    let fn_name = &sig.ident;
    let fn_return_type = &sig.output;
    let fn_block = &item_fn.block;
    let fn_args = &sig.inputs;

    let args = match fn_args
        .iter()
        .map(|arg| {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "test functions cannot have a receiver",
                ));
            };
            Ok((&arg.pat, &arg.ty))
        })
        .collect::<syn::Result<Vec<_>>>()
    {
        Ok(args) => args,
        Err(error) => return error.to_compile_error().into(),
    };

    let arg_defs = args.iter().map(|(pat, ty)| quote!(#pat: #ty));
    let arg_inits = args.iter().map(|(pat, ty)| {
        quote!(<#ty as crate::test_support::FromTag>::from_tag(stringify!(#pat)))
    });

    quote! {
        #( #attrs )*
        #[test]
        fn #fn_name() #fn_return_type {
            crate::test_support::reset_test_state();
            let test = | #( #arg_defs ),* | #fn_block;
            test( #( #arg_inits ),* )
        }
    }
    .into()
}
