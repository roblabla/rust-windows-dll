use proc_macro::TokenStream;
use syn::{
    Result,
    parse,
    Lit,
    LitInt,
    LitStr,
    ItemForeignMod,
    ForeignItem,
    ForeignItemFn,
    FnDecl,
    Meta,
    NestedMeta,
};
use quote::quote;

pub fn parse_windows_dll(metadata: TokenStream, input: TokenStream) -> Result<proc_macro2::TokenStream> {
    let dll_name = parse_dll_name(metadata)?;
    Ok(quote! {
        use std::mem::transmute;

        use wchar::wch_c;
        use winapi::{
            shared::{
                ntdef::LPCWSTR,
                minwindef::{
                    LPCVOID,
                    BOOL,
                    WORD,
                    DWORD,
                    FARPROC,
                },
            },
            um::{
                libloaderapi::{LoadLibraryW, GetProcAddress},
                winuser::MAKEINTRESOURCEA,
            },
        };
        #[inline]
        unsafe fn undocumented_winapi(dll_name: LPCWSTR, entry_ordinal: WORD) -> FARPROC {
            let lib = LoadLibraryW(dll_name);

            GetProcAddress(lib, MAKEINTRESOURCEA(entry_ordinal))
        }
    })
}

pub fn parse_dll_name(metadata: TokenStream) -> Result<String> {
    let dll_name_token: LitStr = parse(metadata)?;
    Ok(dll_name_token.value())
}

pub fn parse_extern_block(dll_name: &str, input: TokenStream) -> Result<()> {
    let ItemForeignMod { abi, items, .. } = parse(input)?;

    let functions = items.into_iter().map(|i| {
        match i {
            ForeignItem::Fn(ForeignItemFn { attrs, vis, ident, decl, .. }) => {
                let attr = attrs.iter().find_map(|attr| {
                    let meta = attr.parse_meta().ok()?;
                    if meta.name().to_string() == "link_ordinal" {
                        Some(meta)
                    } else {
                        None
                    }
                });
                let link_ordinal = match attr {
                    Some(Meta::List(mut list)) => {
                        if list.nested.len() == 1 {
                            list
                                .nested
                                .pop()
                                .and_then(|pair| {
                                    match pair.into_value() {
                                        NestedMeta::Literal(Lit::Int(int)) => Some(int),
                                        _ => None,
                                    }
                                })
                        } else {
                            None
                        }
                    },
                    Some(Meta::NameValue(name_value)) => {
                        match name_value.lit {
                            Lit::Int(int) => Some(int),
                            _ => None,
                        }
                    },
                    _ => None,
                };
                let FnDecl { generics, inputs, variadic, output, .. } = &*decl;
                quote! {
                    unsafe fn #generics () #output {
                        let func = undocumented_winapi(wch_c!(#dll_name).as_ptr(), #link_ordinal);
                        let func: unsafe extern #abi fn() #output = transmute(func);

                        func();
                    }
                }
            },
            _ => panic!("Not a function"),
        }
    });
    Ok(())
}
