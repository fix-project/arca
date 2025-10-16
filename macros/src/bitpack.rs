use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Ident};

pub fn bitpack(input: TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    match input.data {
        Data::Enum(de) => bitpack_enum(&name, de),
        Data::Struct(_) => compile_error("Unable to create bitpack for struct"),
        Data::Union(_) => compile_error("Unable to create bitpack for union"),
    }
}

fn compile_error(msg: &str) -> TokenStream {
    syn::Error::new(proc_macro2::Span::call_site(), msg)
        .to_compile_error()
        .into()
}

const fn ceil_log2(n: u32) -> u32 {
    if n <= 1 {
        0
    } else {
        32 - (n - 1).leading_zeros()
    }
}

struct Variant {
    index: u32,
    pat: proc_macro2::TokenStream,
    construct: proc_macro2::TokenStream,
    width: proc_macro2::TokenStream,
    unpack: proc_macro2::TokenStream,
}

fn bitpack_enum(name: &Ident, de: DataEnum) -> TokenStream {
    let mut variants = Vec::new();
    for (index, v) in de.variants.iter().enumerate() {
        let ident = v.ident.clone();

        let ty = match &v.fields {
            syn::Fields::Named(fields_named) => {
                if fields_named.named.len() != 1 {
                    return compile_error("Unable to create bitpack for variants not of 1 field");
                }
                &fields_named.named.first().unwrap().ty
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                if fields_unnamed.unnamed.len() != 1 {
                    return compile_error("Unable to create bitpack for variants not of 1 field");
                }
                &fields_unnamed.unnamed.first().unwrap().ty
            }
            syn::Fields::Unit => {
                return compile_error("Unable to create bitpack for variants not of 1 field")
            }
        };

        let pat = quote! { #name::#ident(inner) };
        let construct = quote! { Self::#ident };
        let width = quote! { #ty::TAGBITS };
        let unpack = quote! { #ty::unpack };

        variants.push(Variant {
            index: index as u32,
            pat,
            construct,
            width,
            unpack,
        })
    }

    let child_widths = variants.iter().map(|v| &v.width);
    let max_child_widths = quote! {
        {
            let mut m: u32 = 0;
            #( {
                let w = #child_widths; if w > m { m = w; }
            })*
            m
        }
    };
    let curr_width = ceil_log2(variants.len().try_into().unwrap());

    let tag_bits = quote! { #max_child_widths + #curr_width };
    let tag_mask = quote! { bitmask256::<#max_child_widths, #curr_width>() };

    let unpack_arms = variants.iter().map(|v| {
        let index: u64 = v.index.into();
        let construct = &v.construct;
        let unpack = &v.unpack;
        quote! {
            #index => { #construct( #unpack( content )) }
        }
    });

    let pack_arms = variants.iter().map(|v| {
        let index = v.index;
        let pat = &v.pat;
        quote! {
            #pat => {
                let mut result = inner.pack();
                result &= !Self::TAGMASK;
                let field: &mut u16x16 = unsafe { core::mem::transmute( &mut result ) };
                field[15] |= (#index << (Self::TAGBITS - 240 - 1)) as u16;
                result
            }
        }
    });

    let output = quote! {
        impl #name {
            const TAGMASK: u8x32 = #tag_mask;
        }

        impl BitPack for #name {
            const TAGBITS: u32 = #tag_bits;

            fn pack(&self) -> u8x32 {
                match self {
                    #(#pack_arms)*
                }
            }

            fn unpack(content: u8x32) -> Self {
                let tag = content & Self::TAGMASK;
                let field: &u16x16 = unsafe { core::mem::transmute( &tag ) };
                let tag = field[15] >> (Self::TAGBITS - 240 - 1);
                match tag as u64 {
                    #(#unpack_arms)*
                    _ => todo!()
                }
            }

        }
    };
    output.into()
}
