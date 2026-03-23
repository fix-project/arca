use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Ident};

fn common_ident() -> Ident {
    let found_crate = crate_name("common").expect("common is present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => format_ident!("crate"),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

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
    let common = common_ident();
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
                use #common::bitpack::BitPack;
                let mut result = inner.pack();
                for i in 0..32 {
                    result[i] &= !Self::TAGMASK[i];
                }
                let field: &mut [u16; 16] = unsafe { core::mem::transmute( &mut result ) };
                field[15] |= (#index << (Self::TAGBITS - 240 - 1)) as u16;
                result
            }
        }
    });

    let output = quote! {
        impl #name {
            const TAGMASK: [u8; 32] = #tag_mask;
        }

        impl #common::bitpack::BitPack for #name {
            const TAGBITS: u32 = #tag_bits;

            fn pack(&self) -> [u8; 32] {
                match self {
                    #(#pack_arms)*
                }
            }

            fn unpack(content: [u8; 32]) -> Self {
                let mut tag = content;
                for i in 0..32 {
                    tag[i] &= Self::TAGMASK[i];
                }
                let field: &[u16; 16] = unsafe { core::mem::transmute( &tag ) };
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
