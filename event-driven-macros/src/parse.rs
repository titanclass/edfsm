use std::mem;

use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2, Error, Ident, ImplItem, ImplItemMacro, ImplItemType, ItemImpl, Result, Token, Type,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Entry {
    pub state: Type,
}

impl Parse for Entry {
    fn parse(input: ParseStream) -> Result<Self> {
        let state = input.parse()?;
        input.parse::<Token![/]>()?;
        let ident = input.parse::<Ident>()?;
        let ident_str = ident.to_string();
        if ident_str != "entry" {
            return Err(Error::new_spanned(ident, format!("Unknown state qualifer: `/ {ident_str}`. Use only `/ entry` to indicate entry points here.")));
        };
        Ok(Self { state })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct TargetStates {
    pub states: Vec<Type>,
}

impl Parse for TargetStates {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut target_types = vec![];
        loop {
            let target_type = input.parse()?;
            target_types.push(target_type);
            if input.parse::<Token![|]>().is_err() {
                break;
            }
        }
        Ok(Self {
            states: target_types,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Transition {
    pub from_state: Type,
    pub command: Type,
    pub event: Option<Type>,
    pub to_state: Option<TargetStates>,
}

impl Parse for Transition {
    fn parse(input: ParseStream) -> Result<Self> {
        let from_state = input.parse()?;
        input.parse::<Token![=>]>()?;
        let command = input.parse()?;
        let (event, to_state) = if input.parse::<Token![=>]>().is_ok() {
            let event = Some(input.parse()?);
            let to_state = if input.parse::<Token![=>]>().is_ok() {
                Some(input.parse()?)
            } else {
                None
            };
            (event, to_state)
        } else {
            (None, None)
        };
        Ok(Self {
            from_state,
            command,
            event,
            to_state,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Ignore {
    pub from_state: Type,
    pub command: Type,
}

impl Parse for Ignore {
    fn parse(input: ParseStream) -> Result<Self> {
        let from_state = input.parse()?;
        input.parse::<Token![=>]>()?;
        let command = input.parse()?;
        Ok(Self {
            from_state,
            command,
        })
    }
}

#[derive(Debug)]
pub struct Fsm {
    pub state_enum: Type,
    pub command_enum: Type,
    pub event_enum: Type,
    pub effect_handlers: Type,
    pub entry_handlers: Vec<Entry>,
    pub transitions: Vec<Transition>,
    pub ignores: Vec<Ignore>,
    pub item_impl: ItemImpl,
}

impl Parse for Fsm {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut item_impl = input.parse::<ItemImpl>()?;

        let items = mem::take(&mut item_impl.items);

        let mut state_enum = None;
        let mut command_enum = None;
        let mut event_enum = None;
        let mut effect_handlers = None;
        let mut entry_handlers = vec![];
        let mut transitions = vec![];
        let mut ignores = vec![];

        for item in items {
            match item {
                ImplItem::Type(ImplItemType { ident, ty, .. }) => {
                    let type_name = quote!(#ident).to_string();
                    match type_name.as_str() {
                        "S" => {
                            state_enum = Some(ty);
                        }
                        "C" => {
                            command_enum = Some(ty);
                        }
                        "E" => {
                            event_enum = Some(ty);
                        }
                        "SE" => {
                            effect_handlers = Some(ty);
                        }
                        n => {
                            return Err(Error::new_spanned(ident, format!("Unknown associated types: `{n}!`. Use only `S`, `C`, `E` and `SE` here.")));
                        }
                    }
                }
                ImplItem::Macro(ImplItemMacro { mac, .. }) => {
                    let path = mac.path.clone();
                    let macro_name = quote!(#path).to_string();
                    match macro_name.as_str() {
                        "state" => {
                            entry_handlers.push(parse2(mac.tokens)?);
                        }
                        "transition" => {
                            transitions.push(parse2::<Transition>(mac.tokens)?);
                        }
                        "ignore" => {
                            ignores.push(parse2::<Ignore>(mac.tokens)?);
                        }
                        n => {
                            return Err(Error::new_spanned(mac, format!("Unknown macro: `{n}!`. Use only `state!`, `transition!` and `ignore!` macros here.")));
                        }
                    }
                }
                _ => {
                    return Err(Error::new_spanned(
                        item,
                        "Unexpected. Use only the associated type declarations, and `state!`, `transition!` and `ignore!` macros here.",
                    ));
                }
            }
        }
        if let (Some(state_enum), Some(command_enum), Some(event_enum), Some(effect_handlers)) =
            (state_enum, command_enum, event_enum, effect_handlers)
        {
            Ok(Self {
                state_enum,
                command_enum,
                event_enum,
                effect_handlers,
                entry_handlers,
                transitions,
                ignores,
                item_impl,
            })
        } else {
            Err(Error::new_spanned(
                item_impl,
                "Unexpected. Missing one or more associated types: `{n}!`. Declare all of `S`, `C`, `E` and `SE` here.",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_parse() {
        let fsm = parse2::<Fsm>(quote!(
            impl<'d, R> Fsm for Configurator<'d, R>
            where
                R: RngCore + 'd,
            {
                type S = State;
                type C = Command;
                type E = Event;
                type SE = EffectHandlers<'d, R>;

                state!(Uninitialised / entry);

                transition!(Uninitialised  => GenerateRootKey => RootKeyGenerated => SsInitialised);
                transition!(SsInitialised  => GenerateVpnKey  => VpnKeyGenerated  => VpnInitialised);
                transition!(VpnInitialised => SetCredentials  => CredentialsSet   => Configurable);
                transition!(_              => GetUsername);
                transition!(_              => Reset           => FactoryReset     => Uninitialised);
                transition!(_              => Reset           => SoftReset        => VpnInitialised);
            }
        )).unwrap();

        assert_eq!(fsm.state_enum, parse2(quote!(State)).unwrap());
        assert_eq!(fsm.command_enum, parse2(quote!(Command)).unwrap());
        assert_eq!(fsm.event_enum, parse2(quote!(Event)).unwrap());
        assert_eq!(
            fsm.effect_handlers,
            parse2(quote!(EffectHandlers<'d, R>)).unwrap()
        );

        assert_eq!(
            fsm.entry_handlers,
            [parse2(quote!(Uninitialised / entry)).unwrap()]
        );

        assert_eq!(
            fsm.transitions,
            [
                parse2(
                    quote!(Uninitialised  => GenerateRootKey => RootKeyGenerated => SsInitialised)
                )
                .unwrap(),
                parse2(
                    quote!(SsInitialised  => GenerateVpnKey  => VpnKeyGenerated  => VpnInitialised)
                )
                .unwrap(),
                parse2(
                    quote!(VpnInitialised => SetCredentials  => CredentialsSet   => Configurable)
                )
                .unwrap(),
                parse2(quote!(_              => GetUsername)).unwrap(),
                parse2(
                    quote!(_              => Reset           => FactoryReset     => Uninitialised)
                )
                .unwrap(),
                parse2(
                    quote!(_              => Reset           => SoftReset        => VpnInitialised)
                )
                .unwrap(),
            ]
        );
    }

    #[test]
    fn test_multi_targets() {
        let fsm = parse2::<Fsm>(quote!(
            impl Fsm for SomeFsm {
                type S = State;
                type C = Command;
                type E = Event;
                type SE = EffectHandlers<'d, R>;
                transition!(S0  => C => E => S0 | S1);
            }
        ))
        .unwrap();

        assert_eq!(
            fsm.transitions[0].to_state.as_ref().unwrap().states.len(),
            2
        );
    }
}
