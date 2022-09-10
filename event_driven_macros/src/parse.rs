use std::mem;

use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2, Error, Ident, ImplItem, ImplItemMacro, ItemImpl, Result, Token, Type,
};

#[derive(Debug, Eq, PartialEq)]
pub struct EntryExit {
    pub is_entry: bool,
    pub state: Type,
}

impl Parse for EntryExit {
    fn parse(input: ParseStream) -> Result<Self> {
        let state = input.parse()?;
        input.parse::<Token![/]>()?;
        let ident = input.parse::<Ident>()?;
        let ident_str = ident.to_string();
        let is_entry = if ident_str == "entry" {
            true
        } else if ident_str == "exit" {
            false
        } else {
            return Err(Error::new_spanned(ident, format!("Unknown state qualifer: `/ {ident_str}`. Use only `/ entry` and `/ exit` to indicate entry and exit points here.")));
        };
        Ok(Self { is_entry, state })
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

#[derive(Debug)]
pub struct Fsm {
    pub entry_exit_handlers: Vec<EntryExit>,
    pub transitions: Vec<Transition>,
    pub item_impl: ItemImpl,
}

impl Parse for Fsm {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut item_impl = input.parse::<ItemImpl>()?;

        let items = mem::take(&mut item_impl.items);

        let mut entry_exit_handlers = vec![];
        let mut transitions = vec![];

        for item in items {
            if let ImplItem::Macro(ImplItemMacro { mac, .. }) = item {
                let path = mac.path.clone();
                let macro_name = quote!(#path).to_string();
                match macro_name.as_str() {
                    "state" => {
                        entry_exit_handlers.push(parse2(mac.tokens)?);
                    }
                    "transition" => {
                        transitions.push(parse2::<Transition>(mac.tokens)?);
                    }
                    n => {
                        return Err(Error::new_spanned(mac, format!("Unknown macro: `{n}!`. Use only `state!` and `transition!` macros here.")));
                    }
                }
            } else {
                return Err(Error::new_spanned(
                    item,
                    "Unexpected. Use only `state!` and `transition!` macros here.",
                ));
            }
        }

        Ok(Self {
            entry_exit_handlers,
            transitions,
            item_impl,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_parse() {
        let fsm = parse2::<Fsm>(quote!(
            impl<'d, R> Fsm<State, Command, Event, EffectHandlers<'d, R>> for Configurator<'d, R>
            where
                R: RngCore + 'd,
            {
                state!(Uninitialised / exit);

                transition!(Uninitialised  => GenerateRootKey => RootKeyGenerated => SsInitialised);
                transition!(SsInitialised  => GenerateVpnKey  => VpnKeyGenerated  => VpnInitialised);
                transition!(VpnInitialised => SetCredentials  => CredentialsSet   => Configurable);
                transition!(_              => GetUsername);
                transition!(_              => Reset           => FactoryReset     => Uninitialised);
                transition!(_              => Reset           => SoftReset        => VpnInitialised);
            }
        )).unwrap();

        assert_eq!(
            fsm.entry_exit_handlers,
            [parse2(quote!(Uninitialised / exit)).unwrap()]
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
            impl Fsm<State, Command, Event, EffectHandlers> for SomeFsm {
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
