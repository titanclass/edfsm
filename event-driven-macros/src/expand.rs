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

    let mut entry_matches = Vec::with_capacity(fsm.entry_exit_handlers.len());
    let mut exit_matches = Vec::with_capacity(fsm.entry_exit_handlers.len());
    for ee in &fsm.entry_exit_handlers {
        let state = ident_from_type(&ee.state)?;
        if ee.is_entry {
            let handler = format_ident!("on_entry_{}", state);
            let handler = Ident::new(&handler.to_string().to_lowercase(), handler.span());
            entry_matches.push(quote!(
                #state_enum::#state(s) => Self::#handler(s, se).await,
            ));
        } else {
            let handler = format_ident!("on_exit_{}", state);
            let handler = Ident::new(&handler.to_string().to_lowercase(), handler.span());
            exit_matches.push(quote!(
                #state_enum::#state(s) => Self::#handler(s, se).await,
            ));
        };
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
        let (to_state, to_state_explicit) = if let Some(to_state) = &t.to_state {
            match to_state.states.as_slice() {
                [single_type] => (Some(ident_from_type(single_type)?), true),
                [first_type, ..] => (Some(ident_from_type(first_type)?), false),
                _ => panic!("There must be at least one element"),
            }
        } else {
            (None, false)
        };

        if let Some(from_state) = from_state {
            let command_handler = lowercase_ident(&format_ident!("for_{}_{}", from_state, command));
            if let Some(event) = event {
                command_matches.push(quote!(
                    (#state_enum::#from_state(s), #command_enum::#command(c)) => {
                        Self::#command_handler(s, c, se).await.map(|r| #event_enum::#event(r))
                    }
                ));
            } else {
                command_matches.push(quote!(
                    (#state_enum::#from_state(s), #command_enum::#command(c)) => {
                        Self::#command_handler(s, c, se).await;
                        None
                    }
                ));
            }
        } else {
            let command_handler = lowercase_ident(&format_ident!("for_any_{}", command));
            if let Some(event) = event {
                command_matches.push(quote!(
                    (_, #command_enum::#command(c)) => {
                        Self::#command_handler(s, c, se).await.map(|r| #event_enum::#event(r))
                    }
                ));
            } else {
                command_matches.push(quote!(
                    (_, #command_enum::#command(c)) => {
                        Self::#command_handler(s, c, se).await;
                        None
                    }
                ));
            }
        }

        if let Some(to_state) = to_state {
            if let Some(from_state) = from_state {
                if let Some(event) = event {
                    let event_handler =
                        lowercase_ident(&format_ident!("on_{}_{}", from_state, event));
                    if to_state_explicit {
                        event_matches.push(quote!(
                            (#state_enum::#from_state(s), #event_enum::#event(e)) => {
                                Self::#event_handler(s, e).map(|r| #state_enum::#to_state(r))
                            }
                        ));
                    } else {
                        event_matches.push(quote!(
                            (#state_enum::#from_state(s), #event_enum::#event(e)) => {
                                Self::#event_handler(s, e)
                            }
                        ));
                    }
                }
            } else {
                let event = event.unwrap(); // Logic error if no event given a to_state.
                let event_handler = lowercase_ident(&format_ident!("on_any_{}", event));
                if to_state_explicit {
                    event_matches.push(quote!(
                        (_, #event_enum::#event(e)) => {
                            Self::#event_handler(s, e).map(|r| #state_enum::#to_state(r))
                        }
                    ));
                } else {
                    event_matches.push(quote!(
                        (_, #event_enum::#event(e)) => {
                            Self::#event_handler(s, e)
                        }
                    ));
                }
            };
        } else if from_state.is_none() {
            // from and to states are None
            if let Some(event) = event {
                let event_handler = lowercase_ident(&format_ident!("on_any_{}", event));
                event_matches.push(quote!(
                    (_, #event_enum::#event(e)) => {
                        Self::#event_handler(s, e)
                    }
                ));
            }
        }
    }

    for i in &fsm.ignores {
        let from_state = if let Type::Infer(_) = i.from_state {
            None
        } else {
            Some(ident_from_type(&i.from_state)?)
        };
        let command = ident_from_type(&i.command)?;

        if let Some(from_state) = from_state {
            command_matches.push(quote!(
                (#state_enum::#from_state(s), #command_enum::#command(c)) => None,
            ));
        } else {
            command_matches.push(quote!(
                (_, #command_enum::#command(c)) => None,
            ));
        }
    }

    fsm.item_impl.items = vec![
        parse2::<ImplItem>(quote!(
            async fn for_command(
                s: &#state_enum,
                c: #command_enum,
                se: &mut #effect_handlers,
            ) -> Option<#event_enum> {
                match (s, c) {
                    #( #command_matches )*
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn on_event(
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
            async fn on_entry(new_s: &#state_enum, se: &mut #effect_handlers) {
                match new_s {
                    #( #entry_matches )*
                    _ => {}
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            async fn on_exit(old_s: &#state_enum, se: &mut #effect_handlers) {
                match old_s {
                    #( #exit_matches )*
                    _ => {}
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn is_transitioning(s0: &#state_enum, s1: &#state_enum) -> bool {
                core::mem::discriminant(s0) != core::mem::discriminant(s1)
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
