use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, Ident, ItemFn, Pat, Type, parse_macro_input};

pub fn apply(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let callback = &item.sig.ident;
    quote! {
        #item
        #[unsafe(export_name = "_fix_apply")]
        pub extern "C" fn __fix_apply(combination: u32) -> u32 {
            ::fixwasm::__apply(combination, |combination| {
                ::fixwasm::Value::capture(&#callback(combination)?)
            })
        }
    }
    .into()
}

pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let context = if attr.is_empty() {
        None
    } else {
        Some(parse_macro_input!(attr as Ident))
    };
    let item = parse_macro_input!(item as ItemFn);
    let callback = &item.sig.ident;
    let mut bindings = Vec::new();
    let mut arguments = Vec::new();
    let mut context_binding = None;
    for (index, input) in item.sig.inputs.iter().enumerate() {
        let FnArg::Typed(input) = input else {
            return syn::Error::new_spanned(input, "expected a function argument")
                .to_compile_error()
                .into();
        };
        if index == 0 && context.is_some() {
            if !matches!(input.pat.as_ref(), Pat::Ident(name) if Some(&name.ident) == context.as_ref())
            {
                return syn::Error::new_spanned(input, "the context must be the first parameter")
                    .to_compile_error()
                    .into();
            }
            let ty = &input.ty;
            let (owned, argument) = match ty.as_ref() {
                Type::Reference(reference) => {
                    let owned = reference.elem.as_ref();
                    let argument = if reference.mutability.is_some() {
                        quote!(&mut __fix_context)
                    } else {
                        quote!(&__fix_context)
                    };
                    (owned, argument)
                }
                ty => (ty, quote!(__fix_context)),
            };
            context_binding = Some(quote! {
                let mut __fix_context: #owned = ::fixwasm::Combination::from_combination(
                    ::fixwasm::Value::capture(combination)?
                )?;
            });
            arguments.push(argument);
        } else {
            let name = format_ident!("argument_{index}");
            let ty = &input.ty;
            bindings.push(quote!(let #name: #ty = ::fixwasm::Argument::from_argument(arguments.next().unwrap())?;));
            arguments.push(quote!(#name));
        }
    }
    let count = bindings.len();
    quote! {
        #item
        #[::fixwasm::apply]
        fn __fix_main(combination: &::fixwasm::Handle<::fixwasm::Object<::fixwasm::Tree>>)
            -> Result<::fixwasm::Handle<::fixwasm::Any>, ::fixwasm::Error>
        {
            let result = (|| {
                let Some((arguments, count)) = ::fixwasm::fixed_arguments::<#count>(combination)? else {
                    return ::fixwasm::Value::capture(combination);
                };
                let mut arguments = arguments.into_iter();
                #(#bindings)*
                #context_binding
                let result = #callback(#(#arguments),*);
                if count == #count {
                    ::fixwasm::Value::capture(&result)
                } else {
                    ::fixwasm::apply_excess(combination, result, #count)
                }
            })();
            result.or_else(::fixwasm::error)
        }
    }
    .into()
}
