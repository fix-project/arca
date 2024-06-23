use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemStatic};

pub fn body(_: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStatic);
    let attrs = input.attrs;
    let ty = input.ty;
    let mutability = input.mutability;
    let ident = input.ident;
    let mod_ident = format_ident!("__core_local_{}", ident);
    let struct_ident = format_ident!("__CoreLocal_{}", ident);
    let init = input.expr;
    let vis = input.vis;

    quote! {
        #[allow(non_snake_case)]
        mod #mod_ident {
            use super::*;

            #(#attrs)*
            #[link_section = ".cdata"]
            pub(super) static mut #ident: #ty = #init;
        }

        #[allow(non_camel_case_types)]
        #vis struct #struct_ident;

        impl #struct_ident {
            pub fn with<T>(&self, f: impl FnOnce(&mut #ty) -> T) -> T {
                unsafe {
                    let x = &mut *self.as_ptr();
                    f(x)
                }
            }

            pub fn get(&self) -> &#ty {
                unsafe {&*self.as_ptr()}
            }

            pub fn get_mut(&mut self) -> &mut #ty {
                unsafe {&mut *self.as_ptr()}
            }

            pub fn set(&self, value: #ty) {
                self.with(|x| {
                    *x = value;
                });
            }

            pub fn swap(&self, value: &mut #ty) {
                self.with(|x| {
                    core::mem::swap(x, value);
                });
            }

            pub fn replace(&self, value: #ty) -> #ty {
                self.with(|x| {
                    core::mem::replace(x, value)
                })
            }

            pub fn as_ptr(&self) -> *mut #ty {
                let mut address: *mut #ty;
                let mut gs: *const ();
                unsafe {
                    core::arch::asm!("mov {gs}, gs:[0]", gs=out(reg)gs);
                    core::arch::asm!("lea {address}, {offset}[{gs}]", gs=in(reg)gs, address=out(reg)address, offset=sym #mod_ident::#ident);
                }
                address
            }
        }

        impl core::ops::Deref for #struct_ident {
            type Target = #ty;

            fn deref(&self) -> &Self::Target {
                self.get()
            }
        }

        impl core::ops::DerefMut for #struct_ident {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.get_mut()
            }
        }

        #vis static #mutability #ident: #struct_ident = #struct_ident;
    }

    .into()
}
