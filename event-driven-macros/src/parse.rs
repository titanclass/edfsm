use std::mem;

use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2, token, Error, Ident, ImplItem, ImplItemMacro, ImplItemType, ItemImpl, Result, Type,
};

pub struct Entry {
    pub state: Type,
}

impl Parse for Entry {
    fn parse(input: ParseStream) -> Result<Self> {
        let state = input.parse()?;
        input.parse::<token::Div>()?;
        let ident = input.parse::<Ident>()?;
        let ident_str = ident.to_string();
        if ident_str != "entry" {
            return Err(Error::new_spanned(ident, format!("Unknown state qualifer: `/ {ident_str}`. Use only `/ entry` to indicate entry points here.")));
        };
        Ok(Self { state })
    }
}

#[derive(Clone)]
pub struct TargetStates {
    pub states: Vec<Type>,
}

impl Parse for TargetStates {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut target_types = vec![];
        loop {
            let target_type = input.parse()?;
            target_types.push(target_type);
            if input.parse::<token::Or>().is_err() {
                break;
            }
        }
        Ok(Self {
            states: target_types,
        })
    }
}

pub trait Step {
    #[allow(clippy::wrong_self_convention)]
    fn from_state(&self) -> &Type;
    fn command(&self) -> &Option<Type>;
    fn event(&self) -> &Option<Type>;
    fn to_state(&self) -> &Option<TargetStates>;
    fn on_change(&self) -> bool;
}

pub struct CommandStep {
    pub from_state: Type,
    pub command: Option<Type>,
    pub event: Option<Type>,
    pub to_state: Option<TargetStates>,
}

impl Parse for CommandStep {
    fn parse(input: ParseStream) -> Result<Self> {
        let from_state = input.parse()?;
        input.parse::<token::FatArrow>()?;
        let command = Some(input.parse()?);
        let (event, to_state) = if input.parse::<token::FatArrow>().is_ok() {
            let event = Some(input.parse()?);
            let to_state = if input.parse::<token::FatArrow>().is_ok() {
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

impl Step for CommandStep {
    fn from_state(&self) -> &Type {
        &self.from_state
    }

    fn command(&self) -> &Option<Type> {
        &self.command
    }

    fn event(&self) -> &Option<Type> {
        &self.event
    }

    fn to_state(&self) -> &Option<TargetStates> {
        &self.to_state
    }

    fn on_change(&self) -> bool {
        false
    }
}

pub struct EventStep {
    pub from_state: Type,
    pub command: Option<Type>,
    pub event: Option<Type>,
    pub to_state: Option<TargetStates>,
    pub on_change: bool,
}

impl Parse for EventStep {
    fn parse(input: ParseStream) -> Result<Self> {
        let from_state = input.parse()?;
        input.parse::<token::FatArrow>()?;
        let event = Some(input.parse()?);
        let to_state = if input.peek(token::FatArrow) {
            input.parse::<token::FatArrow>()?;
            Some(input.parse()?)
        } else {
            None
        };
        let on_change = if input.peek(token::Div) {
            input.parse::<token::Div>()?;
            let ident = input.parse::<Ident>()?;
            let ident_str = ident.to_string();
            if ident_str != "action" {
                return Err(Error::new_spanned(ident, format!("Unknown state qualifer: `/ {ident_str}`. Use only `/ action` to indicate there is going to be an action here.")));
            };
            true
        } else {
            false
        };

        Ok(Self {
            from_state,
            command: None,
            event,
            to_state,
            on_change,
        })
    }
}

impl Step for EventStep {
    fn from_state(&self) -> &Type {
        &self.from_state
    }

    fn command(&self) -> &Option<Type> {
        &self.command
    }

    fn event(&self) -> &Option<Type> {
        &self.event
    }

    fn to_state(&self) -> &Option<TargetStates> {
        &self.to_state
    }

    fn on_change(&self) -> bool {
        self.on_change
    }
}

pub struct IgnoreCommand {
    pub from_state: Type,
    pub command: Type,
}

impl Parse for IgnoreCommand {
    fn parse(input: ParseStream) -> Result<Self> {
        let from_state = input.parse()?;
        input.parse::<token::FatArrow>()?;
        let command = input.parse()?;
        Ok(Self {
            from_state,
            command,
        })
    }
}

pub struct IgnoreEvent {
    pub from_state: Type,
    pub event: Type,
}

impl Parse for IgnoreEvent {
    fn parse(input: ParseStream) -> Result<Self> {
        let from_state = input.parse()?;
        input.parse::<token::FatArrow>()?;
        let event = input.parse()?;
        Ok(Self { from_state, event })
    }
}

pub struct Fsm {
    pub state_enum: Type,
    pub command_enum: Type,
    pub event_enum: Type,
    pub effect_handlers: Type,
    pub entry_handlers: Vec<Entry>,
    pub steps: Vec<Box<dyn Step>>,
    pub ignore_commands: Vec<IgnoreCommand>,
    pub ignore_events: Vec<IgnoreEvent>,
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
        let mut steps = vec![];
        let mut ignore_commands = vec![];
        let mut ignore_events = vec![];

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
                        "command" => {
                            steps.push(Box::new(parse2::<CommandStep>(mac.tokens)?) as Box<dyn Step>);
                        }
                        "event" => {
                            steps.push(Box::new(parse2::<EventStep>(mac.tokens)?) as Box<dyn Step>);
                        }
                        "ignore_command" => {
                            ignore_commands.push(parse2::<IgnoreCommand>(mac.tokens)?);
                        }
                        "ignore_event" => {
                            ignore_events.push(parse2::<IgnoreEvent>(mac.tokens)?);
                        }
                        n => {
                            return Err(Error::new_spanned(mac, format!("Unknown macro: `{n}!`. Use only `state!`, `command!`, `event!`, `ignore_command!` and `ignore_event!` macros here.")));
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
                steps,
                ignore_commands,
                ignore_events,
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
