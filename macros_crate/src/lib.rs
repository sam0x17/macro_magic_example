use macro_magic::import_tokens_proc;
use proc_macro::TokenStream;

#[import_tokens_proc]
#[proc_macro]
pub fn print_foreign_item(tokens: TokenStream) -> TokenStream {
    println!("{}", tokens.to_string());
    "".parse().unwrap()
}
