use proc_macro::TokenStream;

mod parse;
use proc_macro_error::{abort_call_site, proc_macro_error};
use quote::quote;
use quote::ToTokens;
use syn::{parse2, ImplItem};

#[proc_macro_attribute]
#[proc_macro_error]
pub fn impl_fsm(input: TokenStream, annotated_item: TokenStream) -> TokenStream {
    if !input.is_empty() {
        abort_call_site!("this attribute takes no arguments"; help = "use `#[impl-fsm]`")
    }

    match parse2::<parse::Fsm>(annotated_item.into()) {
        Ok(mut fsm) => {
            fsm.item_impl.items = vec![
                parse2::<ImplItem>(quote!(
                    fn for_command(
                        s: &State,
                        c: &Command,
                        se: &mut EffectHandlers,
                    ) -> Option<Event> {
                        todo!()
                    }
                ))
                .unwrap(),
                parse2::<ImplItem>(quote!(
                    fn for_event(s: &State, e: &Event) -> Option<State> {
                        todo!()
                    }
                ))
                .unwrap(),
            ];
            fsm.item_impl.to_token_stream().into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}
