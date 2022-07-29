use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::Ident;
use syn::Type;
use syn::{parse2, Error, ImplItem, Result};

use crate::parse::Fsm;

pub fn expand(fsm: &mut Fsm) -> Result<TokenStream> {
    let mut command_matches = Vec::with_capacity(fsm.transitions.len());
    let mut event_matches = Vec::with_capacity(fsm.transitions.len());
    for t in &fsm.transitions {
        let from_state = ident_from_type(&t.from_state)?;
        let command = ident_from_type(&t.command)?;
        let event = ident_from_type(&t.event)?;
        let to_state = if let Some(to_state) = &t.to_state {
            Some(ident_from_type(to_state)?)
        } else {
            None
        };

        let command_handler = format_ident!("for_{}_{}_{}", from_state, command, event);
        let command_handler = Ident::new(
            &command_handler.to_string().to_lowercase(),
            command_handler.span(),
        );
        command_matches.push(quote!(
            (State::#from_state(s), Command::#command(c)) => {
                Self::#command_handler(s, c, se).map(|r| Event::#event(r))
            }
        ));

        let event_handler = if let Some(to_state) = to_state {
            format_ident!("for_{}_{}_{}", from_state, event, to_state)
        } else {
            format_ident!("for_{}_{}", from_state, event)
        };
        let event_handler = Ident::new(
            &event_handler.to_string().to_lowercase(),
            event_handler.span(),
        );
        event_matches.push(quote!(
            (State::#from_state(s), Event::#event(e)) => {
                Self::#event_handler(s, e).map(|r| State::#to_state(r))
            }
        ));
    }
    // FIXME: Extract type params for state, command, event and effect handlers from the impl instead of assuming them here
    fsm.item_impl.items = vec![
        parse2::<ImplItem>(quote!(
            fn for_command(
                s: &State,
                c: &Command,
                se: &mut EffectHandlers,
            ) -> Option<Event> {
                match (s, c) {
                    #( #command_matches )*
                    _ => None,
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn for_event(
                s: &State,
                e: &Event,
            ) -> Option<State> {
                match (s, e) {
                    #( #event_matches )*
                    _ => None,
                }
            }
        ))
        .unwrap(),
    ];
    Ok(fsm.item_impl.to_token_stream())
}

fn ident_from_type(from_type: &Type) -> Result<&Ident> {
    if let Type::Path(path) = from_type {
        if let Some(segment) = path.path.segments.first() {
            if segment.arguments.is_empty() {
                Ok(&segment.ident)
            } else {
                Err(Error::new_spanned(
                    segment.ident.clone(),
                    "No arguments are expected",
                ))
            }
        } else {
            Err(Error::new_spanned(path, "No path segments are expected"))
        }
    } else {
        Err(Error::new_spanned(
            from_type.clone(),
            "A type that can also be expressed as an enum variant is expected",
        ))
    }
}

// TODO: write some tests here
