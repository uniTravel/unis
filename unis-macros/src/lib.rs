//! # **unis** 宏
//!
//!

use proc_macro::TokenStream;
use quote::quote;
use syn::{Fields, Ident, ItemEnum, ItemStruct, parse_macro_input};

/// 规范聚合结构体定义
///
/// 1. 生成私有的 id、revision 字段。
/// 2. 添加 #[derive(Debug, Clone)]。
/// 3. 实现 Aggregate 特征。
///
/// # Panics
///
/// 自定义字段出现 id、revision 会与宏生成的字段冲突。
///
/// 仅支持具名字段的结构体。
///
/// 禁止与 #[derive] 同时使用。
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

    if let Fields::Named(ref fields) = input.fields {
        if fields
            .named
            .iter()
            .any(|f| f.ident.as_ref().unwrap() == "id")
        {
            panic!("结构体`{}`已存在`id`字段", struct_name);
        }
    }

    if let Fields::Named(ref fields) = input.fields {
        if fields
            .named
            .iter()
            .any(|f| f.ident.as_ref().unwrap() == "revision")
        {
            panic!("结构体`{}`已存在`revision`字段", struct_name);
        }
    }

    if let Fields::Named(ref mut fields) = input.fields {
        fields.named.insert(0, syn::parse_quote! (id: ::uuid::Uuid));
        fields.named.insert(1, syn::parse_quote!(revision: u64));
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
        #[derive(Debug, Clone)]
        #input

        impl unis::domain::Aggregate for #struct_name {
            fn new(id: uuid::Uuid) -> Self {
                Self {
                    id,
                    revision: u64::MAX,
                    #(#field_names: Default::default(),)*
                }
            }
            fn next(&mut self) {
                self.revision = self.revision.wrapping_add(1);
            }
            fn id(&self) -> ::uuid::Uuid { self.id }
            fn revision(&self) -> u64 { self.revision }
        }
    };

    TokenStream::from(expanded)
}

/// 规范命令结构体定义
///
/// 1. 添加 #[derive(Debug, ::validator::Validate, ::bincode::Encode, ::bincode::Decode)]。
#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let expanded = quote! {
        #[derive(Debug, ::validator::Validate, ::bincode::Encode, ::bincode::Decode)]
        #input
    };
    TokenStream::from(expanded)
}

/// 规范事件结构体定义
///
/// 1. 添加 #[derive(Debug, ::bincode::Encode, ::bincode::Decode)]。
#[proc_macro_attribute]
pub fn event(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let expanded = quote! {
        #[derive(Debug, ::bincode::Encode, ::bincode::Decode)]
        #input
    };
    TokenStream::from(expanded)
}

/// 规范命令枚举定义
///
/// 1. 添加 #[derive(Debug, ::bincode::Encode, ::bincode::Decode)]。
/// 2. 添加 #[repr(u8)]。
/// 3. 生成基于泛型常量静态分发所需内容。
///
/// # Panics
///
/// 枚举变体需要添加判别值。
#[proc_macro_attribute]
pub fn command_enum(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);
    let enum_name = &input.ident;
    let agg_name = parse_macro_input!(attr as Ident);

    let variants = &input.variants;
    let match_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let disc = variant
            .discriminant
            .as_ref()
            .map(|(_, expr)| quote! { #expr })
            .unwrap_or_else(|| {
                panic!("枚举变体 {} 缺少判别值", variant_name);
            });
        quote! {
            #disc => com.process(agg)
        }
    });

    let expanded = quote! {
        #[derive(Debug, ::bincode::Encode, ::bincode::Decode)]
        #[repr(u8)]
        #input

        impl unis::domain::CommandEnum for #enum_name {
            type A = #agg_name;
        }

        struct Dispatcher<const ID: usize> {}
        impl<const ID: usize> Dispatcher<ID> {
            const fn new() -> Self {
                Self {}
            }

            #[inline(always)]
            fn execute<C, E>(&self, com: C, agg: &mut #agg_name) -> Result<E, DomainError>
            where
                C: unis::domain::Command<A = #agg_name, E = E>,
                E: unis::domain::Event<A = #agg_name>,
            {
                match ID {
                    #(#match_arms,)*
                    _ => unsafe { std::hint::unreachable_unchecked() },
                }
            }
        }
    };
    TokenStream::from(expanded)
}

/// 规范事件枚举定义
///
/// 1. 添加 #[derive(Debug, ::bincode::Encode, ::bincode::Decode)]。
/// 2. 添加 #[repr(u8)]。
/// 3. 生成重播事件所需内容。
///
/// # Panics
///
/// 枚举变体需要添加判别值。
#[proc_macro_attribute]
pub fn event_enum(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);
    let enum_name = &input.ident;
    let agg_name = parse_macro_input!(attr as Ident);

    let variants = &input.variants;
    let match_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let _ = variant
            .discriminant
            .as_ref()
            .map(|(_, expr)| quote! { #expr })
            .unwrap_or_else(|| {
                panic!("枚举变体 {} 缺少判别值", variant_name);
            });
        quote! {
            #enum_name::#variant_name(evt) => evt.apply(agg)
        }
    });
    let expanded = quote! {
        #[derive(Debug, ::bincode::Encode, ::bincode::Decode)]
        #[repr(u8)]
        #input

        impl unis::domain::EventEnum for #enum_name {
            type A = #agg_name;
        }

        pub struct Replayer;
        impl unis::domain::Replay for Replayer {
            type A = #agg_name;

            fn replay(&self, agg: &mut Self::A, evt_data: Vec<u8>) -> Result<(), DomainError> {
                let (evt, _): (#enum_name, _) = bincode::decode_from_slice(&evt_data, BINCODE_CONFIG)?;
                match evt {
                    #(#match_arms,)*
                }
                Ok(())
            }
        }
    };
    TokenStream::from(expanded)
}
