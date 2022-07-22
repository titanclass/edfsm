use std::mem;

use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2,
    token::If,
    Error, Expr, Ident, ImplItem, ImplItemMacro, ItemImpl, Pat, Path, Result, Token,
};

#[derive(Debug)]
pub struct ArmLhs {
    pub pat: Pat,
    pub guard: Option<(If, Box<Expr>)>,
}

impl Parse for ArmLhs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            pat: input.parse()?,
            guard: {
                if input.peek(Token![if]) {
                    let if_token: Token![if] = input.parse()?;
                    let guard: Expr = input.parse()?;
                    Some((if_token, Box::new(guard)))
                } else {
                    None
                }
            },
        })
    }
}

pub type Command = ArmLhs;
pub type Event = ArmLhs;
pub type State = ArmLhs;

#[derive(Debug, PartialEq)]
pub struct EntryExit {
    is_entry: bool,
    state: Path,
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

#[derive(Debug)]
pub struct Fsm {
    pub command_handlers: Vec<(State, (Command, Event))>,
    pub entry_exit_handlers: Vec<EntryExit>,
    pub event_handlers: Vec<(State, (Event, State))>,
    pub item_impl: ItemImpl,
}

impl Parse for Fsm {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut item_impl = input.parse::<ItemImpl>()?;

        let items = mem::take(&mut item_impl.items);

        let command_handlers = vec![];
        let mut entry_exit_handlers = vec![];
        let event_handlers = vec![];

        for item in items {
            if let ImplItem::Macro(ImplItemMacro { mac, .. }) = item {
                let path = mac.path.clone();
                let macro_name = quote!(#path).to_string();
                match macro_name.as_str() {
                    "state" => {
                        entry_exit_handlers.push(parse2(mac.tokens)?);
                    }
                    "transition" => {}
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
            command_handlers,
            entry_exit_handlers,
            event_handlers,
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
                state!(State::Uninitialised / exit);

                transition!(State::Uninitialised    => Command::GenerateRootKey                       => Event::RootKeyGenerated             => State::RootKeyGenerated);
                transition!(State::RootKeyGenerated => Command::GenerateVpnKey                        => Event::VpnKeyGenerated              => State::VpnKeyGenerated);
                transition!(State::VpnKeyGenerated  => Command::SetCredentials { username, password } => Event::CredentialsSet { username }  => State::CredentialsSet { entity });
                transition!(_                       => Command::GetUsername                           => Event::UsernameRetrieved);
                transition!(_                       => Command::Reset { factory } if factory          => Event::Reset { factory } if factory => State::Uninitialised);
                transition!(_                       => Command::Reset { .. }                          => Event::Reset { .. }                 => State::VpnKeyGenerated);
            }
        )).unwrap();

        assert_eq!(
            fsm.entry_exit_handlers,
            [parse2(quote!(State::Uninitialised / exit)).unwrap()]
        );
    }
}
