use proc_macro2::TokenStream;
use quote::__private::ext::RepToTokensExt;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::Ident;
use syn::PathArguments;
use syn::Type;
use syn::{parse2, Error, ImplItem, Result};

use crate::parse::Fsm;

pub fn expand(fsm: &mut Fsm) -> Result<TokenStream> {
    let (state_enum, command_enum, event_enum, effect_handlers) = if let Some(trait_) =
        &fsm.item_impl.trait_
    {
        let trait_path = &trait_.1;
        if let Some(last_trait_segment) = trait_path.segments.last() {
            if last_trait_segment.ident == "Fsm" {
                if let PathArguments::AngleBracketed(fsm_trait_generic_args) =
                    &last_trait_segment.arguments
                {
                    if fsm_trait_generic_args.args.len() == 4 {
                        let mut args_iter = fsm_trait_generic_args.args.iter();
                        let state_enum = args_iter.next().unwrap();
                        let command_enum = args_iter.next().unwrap();
                        let event_enum = args_iter.next().unwrap();
                        let effect_handlers = args_iter.next().unwrap();
                        (state_enum, command_enum, event_enum, effect_handlers)
                    } else {
                        return Err(Error::new_spanned(
                        &fsm_trait_generic_args.args,
                        "Expected the trait to be implemented with 4 generics representing State, Command, Event enums and the Event Handler.",
                    ));
                    }
                } else {
                    return Err(Error::new_spanned(
                    &last_trait_segment.arguments,
                    "Expected the trait to be implemented with 4 generics representing State, Command, Event enums and the Event Handler.",
                ));
                }
            } else {
                return Err(Error::new_spanned(
                    &last_trait_segment.ident,
                    "Expected the Fsm trait to be implemented.",
                ));
            }
        } else {
            return Err(Error::new_spanned(
                &trait_path.segments,
                "The first generic representing a State enum is required.",
            ));
        }
    } else {
        return Err(Error::new_spanned(
            &fsm.item_impl,
            "Expected the Fsm trait to be implemented.",
        ));
    };
    let mut entry_exit_matches = Vec::with_capacity(fsm.entry_exit_handlers.len());
    for ee in &fsm.entry_exit_handlers {
        let state = ident_from_type(&ee.state)?;
        let entry_exit_match = if ee.is_entry {
            let handler = format_ident!("to_{}", state);
            let handler = Ident::new(&handler.to_string().to_lowercase(), handler.span());
            quote!(
                (_, #state_enum::#state(s)) => Self::#handler(s, se),
            )
        } else {
            let handler = format_ident!("from_{}", state);
            let handler = Ident::new(&handler.to_string().to_lowercase(), handler.span());
            quote!(
                (#state_enum::#state(s), _) => Self::#handler(s, se),
            )
        };
        entry_exit_matches.push(quote!(#entry_exit_match));
    }

    let mut command_matches = Vec::with_capacity(fsm.transitions.len());
    let mut event_matches = Vec::with_capacity(fsm.transitions.len());
    for t in &fsm.transitions {
        let from_state = if let Type::Infer(_) = t.from_state {
            None
        } else {
            Some(ident_from_type(&t.from_state)?)
        };
        let command = ident_from_type(&t.command)?;
        let event = if let Some(event) = &t.event {
            Some(ident_from_type(event)?)
        } else {
            None
        };
        let to_state = if let Some(to_state) = &t.to_state {
            Some(ident_from_type(to_state)?)
        } else {
            None
        };

        if let Some(from_state) = from_state {
            if let Some(event) = event {
                let command_handler =
                    lowercase_ident(&format_ident!("for_{}_{}_{}", from_state, command, event));
                command_matches.push(quote!(
                    (#state_enum::#from_state(s), #command_enum::#command(c)) => {
                        Self::#command_handler(s, c, se).map(|r| #event_enum::#event(r))
                    }
                ));
            } else {
                let command_handler =
                    lowercase_ident(&format_ident!("for_{}_{}", from_state, command));
                command_matches.push(quote!(
                    (#state_enum::#from_state(s), #command_enum::#command(c)) => {
                        Self::#command_handler(s, c, se);
                        None
                    }
                ));
            }
        } else if let Some(event) = event {
            let command_handler = lowercase_ident(&format_ident!("for_any_{}_{}", command, event));
            command_matches.push(quote!(
                (_, #command_enum::#command(c)) => {
                    Self::#command_handler(c, se).map(|r| #event_enum::#event(r))
                }
            ));
        } else {
            let command_handler = lowercase_ident(&format_ident!("for_any_{}", command));
            command_matches.push(quote!(
                (_, #command_enum::#command(c)) => {
                    Self::#command_handler(c, se);
                    None
                }
            ));
        }

        if let Some(to_state) = to_state {
            if let Some(from_state) = from_state {
                if let Some(event) = event {
                    let event_handler = lowercase_ident(&format_ident!(
                        "for_{}_{}_{}",
                        from_state,
                        event,
                        to_state
                    ));
                    event_matches.push(quote!(
                        (#state_enum::#from_state(s), #event_enum::#event(e)) => {
                            Self::#event_handler(s, e).map(|r| #state_enum::#to_state(r))
                        }
                    ));
                }
            } else if let Some(event) = event {
                let event_handler =
                    lowercase_ident(&format_ident!("for_any_{}_{}", event, to_state));
                event_matches.push(quote!(
                    (_, #event_enum::#event(e)) => {
                        Self::#event_handler(e).map(|r| #state_enum::#to_state(r))
                    }
                ));
            }
        }
    }

    fsm.item_impl.items = vec![
        parse2::<ImplItem>(quote!(
            fn for_command(
                s: &#state_enum,
                c: &#command_enum,
                se: &mut #effect_handlers,
            ) -> Option<#event_enum> {
                match (s, c) {
                    #( #command_matches )*
                    _ => None,
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn for_event(
                s: &#state_enum,
                e: &#event_enum,
            ) -> Option<#state_enum> {
                match (s, e) {
                    #( #event_matches )*
                    _ => None,
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn on_transition(old_s: &#state_enum, new_s: &#state_enum, se: &mut #effect_handlers) {
                match (old_s, new_s) {
                    #( #entry_exit_matches )*
                    _ => {}
                }
            }
        ))
        .unwrap(),
    ];
    Ok(fsm.item_impl.to_token_stream())
}

fn lowercase_ident(ident: &Ident) -> Ident {
    Ident::new(&ident.to_string().to_lowercase(), ident.span())
}

fn ident_from_type(from_type: &Type) -> Result<&Ident> {
    if let Type::Path(path) = from_type {
        if path.path.segments.len() == 1 {
            let segment = path.path.segments.next().unwrap().first().unwrap();
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
