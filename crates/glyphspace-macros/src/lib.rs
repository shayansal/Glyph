use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{
    Expr, ExprLit, ItemFn, Lit, MetaNameValue, Token, parse_macro_input, punctuated::Punctuated,
};

#[proc_macro_attribute]
pub fn glyph_component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn glyph_app(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn lens(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn capability(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = Punctuated::<MetaNameValue, Token![,]>::parse_terminated
        .parse(attr)
        .expect("capability attributes must be name-value pairs");
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.clone();
    let manifest_name = format_ident!("{}_manifest", fn_name);

    let id = required_string(&args, "id");
    let name = required_string(&args, "name");
    let permission = optional_string(&args, "permission");
    let risk = optional_string(&args, "risk").unwrap_or_else(|| "low".to_string());
    let risk_tokens = risk_tokens(&risk);

    let permission_tokens = permission.map_or_else(
        || quote! {},
        |permission| quote! { .permission(#permission) },
    );

    quote! {
        #input

        pub fn #manifest_name() -> glyphspace_core::Capability {
            glyphspace_core::Capability::builder(#id, #name)
                #permission_tokens
                .risk(#risk_tokens)
                .build()
        }
    }
    .into()
}

fn required_string(args: &Punctuated<MetaNameValue, Token![,]>, key: &str) -> String {
    optional_string(args, key)
        .unwrap_or_else(|| panic!("missing required capability attribute {key}"))
}

fn optional_string(args: &Punctuated<MetaNameValue, Token![,]>, key: &str) -> Option<String> {
    args.iter()
        .find(|arg| arg.path.is_ident(key))
        .and_then(|arg| match &arg.value {
            Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) => Some(value.value()),
            _ => None,
        })
}

fn risk_tokens(risk: &str) -> proc_macro2::TokenStream {
    match risk {
        "none" => quote! { glyphspace_core::RiskLevel::None },
        "low" => quote! { glyphspace_core::RiskLevel::Low },
        "medium" => quote! { glyphspace_core::RiskLevel::Medium },
        "high" => quote! { glyphspace_core::RiskLevel::High },
        "critical" => quote! { glyphspace_core::RiskLevel::Critical },
        other => panic!("unknown risk level {other}"),
    }
}
