use proc_macro::TokenStream;
use quote::quote;
use syn::{Fields, ItemStruct, parse_macro_input};

#[proc_macro_attribute]
pub fn aggregate(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;

    if input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("derive"))
    {
        panic!("#[aggregate]与#[derive]禁止同时使用");
    }

    if let syn::Fields::Named(ref fields) = input.fields {
        if fields
            .named
            .iter()
            .any(|f| f.ident.as_ref().unwrap() == "id")
        {
            panic!("结构体`{}`已存在`id`字段", struct_name);
        }
    }

    if let syn::Fields::Named(ref fields) = input.fields {
        if fields
            .named
            .iter()
            .any(|f| f.ident.as_ref().unwrap() == "revision")
        {
            panic!("结构体`{}`已存在`revision`字段", struct_name);
        }
    }

    if let syn::Fields::Named(ref mut fields) = input.fields {
        fields.named.insert(0, syn::parse_quote! (id: ::uuid::Uuid));
        fields.named.insert(1, syn::parse_quote!(pub revision: u64));
    } else {
        panic!("#[aggregate]仅支持具名字段的结构体");
    }

    let field_names = if let Fields::Named(fields) = &input.fields {
        fields
            .named
            .iter()
            .filter(|f| {
                let field_name = f.ident.as_ref().unwrap();
                field_name != "id" && field_name != "revision"
            })
            .map(|f| &f.ident)
            .collect()
    } else {
        vec![]
    };

    let expanded = quote! {
        #[derive(Debug)]
        #input

        impl Aggregate for #struct_name {
            fn new(id: Uuid) -> Self {
                Self {
                    id,
                    revision: u64::MAX,
                    #(#field_names: Default::default(),)*
                }
            }
            fn next(&mut self) {
                self.revision = self.revision.wrapping_add(1);
            }
        }
    };

    TokenStream::from(expanded)
}
