use proc_macro::TokenStream;

mod expand;
mod parse;
use proc_macro_error::{abort_call_site, proc_macro_error};
use syn::parse2;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn impl_fsm(input: TokenStream, annotated_item: TokenStream) -> TokenStream {
    if !input.is_empty() {
        abort_call_site!("this attribute takes no arguments"; help = "use `#[impl-fsm]`")
    }

    match parse2::<parse::Fsm>(annotated_item.into()) {
        Ok(mut fsm) => match expand::expand(&mut fsm) {
            Ok(expanded) => expanded.into(),
            Err(e) => e.to_compile_error().into(),
        },
        Err(e) => e.to_compile_error().into(),
    }
}
