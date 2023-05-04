use macro_magic::import_tokens_proc;
use proc_macro::TokenStream;
use quote::quote;

#[import_tokens_proc]
#[proc_macro]
pub fn make_item_const(tokens: TokenStream) -> TokenStream {
    let item_str = tokens.to_string();
    quote! {
        const ITEM_SRC: &'static str = #item_str;
    }
    .into()
}

#[import_tokens_proc]
#[proc_macro]
pub fn print_foreign_item(tokens: TokenStream) -> TokenStream {
    println!("{}", tokens.to_string());
    "".parse().unwrap()
}
