use proc_macro2::TokenStream;
use quote::__private::ext::RepToTokensExt;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::Ident;
use syn::Type;
use syn::{parse2, Error, ImplItem, Result};

use crate::parse::Fsm;

pub fn expand(fsm: &mut Fsm) -> Result<TokenStream> {
    if let Some(trait_) = &fsm.item_impl.trait_ {
        let trait_path = &trait_.1;
        if let Some(last_trait_segment) = trait_path.segments.last() {
            if last_trait_segment.ident != "Fsm" {
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

    let state_enum = &fsm.state_enum;
    let command_enum = &fsm.command_enum;
    let event_enum = &fsm.event_enum;
    let effect_handlers = &fsm.effect_handlers;

    let mut entry_matches = Vec::with_capacity(fsm.entry_handlers.len());
    for ee in &fsm.entry_handlers {
        let state = ident_from_type(&ee.state)?;
        let handler = format_ident!("on_entry_{}", state);
        let handler = Ident::new(&handler.to_string().to_lowercase(), handler.span());
        entry_matches.push(quote!(
            #state_enum::#state(s) => Self::#handler(s, se),
        ));
    }

    let mut command_matches = Vec::with_capacity(fsm.transitions.len());
    let mut event_matches = Vec::with_capacity(fsm.transitions.len());
    let mut change_matches = Vec::with_capacity(fsm.transitions.len());

    for t in &fsm.transitions {
        let from_state = if let Type::Infer(_) = t.from_state {
            None
        } else {
            Some(ident_from_type(&t.from_state)?)
        };
        let command = if let Type::Infer(_) = t.command {
            None
        } else {
            Some(ident_from_type(&t.command)?)
        };
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

        if let Some(command) = command {
            if let Some(from_state) = from_state {
                let command_handler =
                    lowercase_ident(&format_ident!("for_{}_{}", from_state, command));
                if let Some(event) = event {
                    command_matches.push(quote!(
                        (#state_enum::#from_state(s), #command_enum::#command(c)) => {
                            Self::#command_handler(s, c, se).map(#event_enum::#event)
                        }
                    ));
                } else {
                    command_matches.push(quote!(
                        (#state_enum::#from_state(s), #command_enum::#command(c)) => {
                            Self::#command_handler(s, c, se);
                            None
                        }
                    ));
                }
            } else {
                let command_handler = lowercase_ident(&format_ident!("for_any_{}", command));
                if let Some(event) = event {
                    command_matches.push(quote!(
                        (_, #command_enum::#command(c)) => {
                            Self::#command_handler(s, c, se).map(#event_enum::#event)
                        }
                    ));
                } else {
                    command_matches.push(quote!(
                        (_, #command_enum::#command(c)) => {
                            Self::#command_handler(s, c, se);
                            None
                        }
                    ));
                }
            }
        }

        let mut push_change_matches_conditionally = |to_state, event| {
            if command.is_none() {
                let change_handler = if let Some(to_state) = to_state {
                    lowercase_ident(&format_ident!("on_change_{}_{}", to_state, event))
                } else {
                    lowercase_ident(&format_ident!("on_change_any_{}", event))
                };
                change_matches.push(quote!(
                    (#state_enum::#to_state(s), #event_enum::#event(e)) => {
                        Self::#change_handler(s, e, se)
                    }
                ));
            }
        };

        if let Some(to_state) = to_state {
            if let Some(from_state) = from_state {
                if let Some(event) = event {
                    let event_handler =
                        lowercase_ident(&format_ident!("on_{}_{}", from_state, event));
                    if to_state_explicit {
                        event_matches.push(quote!(
                            (#state_enum::#from_state(s), #event_enum::#event(e)) => {
                                Self::#event_handler(s, e).map(|new_s| (edfsm::Change::Transitioned, Some(#state_enum::#to_state(new_s))))
                            }
                        ));
                    } else {
                        event_matches.push(quote!(
                            (#state_enum::#from_state(s), #event_enum::#event(e)) => {
                                Self::#event_handler(s, e).map(|_| (edfsm::Change::Updated, None))
                            }
                        ));
                    }
                    push_change_matches_conditionally(Some(to_state), event);
                }
            } else {
                let event = event.unwrap(); // Logic error if no event given a to_state.
                let event_handler = lowercase_ident(&format_ident!("on_any_{}", event));
                if to_state_explicit {
                    event_matches.push(quote!(
                        (s, #event_enum::#event(e)) => {
                            Self::#event_handler(s, e).map(|new_s| (edfsm::Change::Transitioned, Some(#state_enum::#to_state(new_s))))
                        }
                    ));
                } else {
                    event_matches.push(quote!(
                        (s, #event_enum::#event(e)) => {
                            Self::#event_handler(s, e).map(|_| (edfsm::Change::Updated, None))
                        }
                    ));
                }
                push_change_matches_conditionally(Some(to_state), event);
            };
        } else if let Some(from_state) = from_state {
            if let Some(event) = event {
                let event_handler = lowercase_ident(&format_ident!("on_{}_{}", from_state, event));
                event_matches.push(quote!(
                    (#state_enum::#from_state(s), #event_enum::#event(e)) => {
                        Self::#event_handler(s, e);
                        Some((edfsm::Change::Updated, None))
                    }
                ));
                push_change_matches_conditionally(Some(from_state), event);
            }
        } else {
            // from and to states are None
            if let Some(event) = event {
                let event_handler = lowercase_ident(&format_ident!("on_any_{}", event));
                event_matches.push(quote!(
                    (s, #event_enum::#event(e)) => {
                        Self::#event_handler(s, e);
                        Some((edfsm::Change::Updated, None))
                    }
                ));
                push_change_matches_conditionally(from_state, event);
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
            type S = #state_enum;
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            type C = #command_enum;
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            type E = #event_enum;
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            type SE = #effect_handlers;
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn for_command(
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
                mut s: &mut #state_enum,
                e: &#event_enum,
            ) -> Option<edfsm::Change> {
                let r = match (&mut s, e) {
                    #( #event_matches )*
                    _ => None,
                };
                if let Some((c, new_s)) = r {
                    if let Some(new_s) = new_s {
                        *s = new_s;
                    }
                    Some(c)
                } else {
                    None
                }
            }
        ))
        .unwrap(),
        parse2::<ImplItem>(quote!(
            fn on_change(new_s: &#state_enum, e: &#event_enum, se: &mut #effect_handlers, change: edfsm::Change) {
                if let edfsm::Change::Transitioned = change {
                    match new_s {
                        #( #entry_matches )*
                        _ => {}
                    }
                }
                match (new_s, e) {
                    #( #change_matches )*
                    _ => (),
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
