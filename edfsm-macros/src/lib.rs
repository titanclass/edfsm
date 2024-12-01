use proc_macro::TokenStream;

mod expand;
mod parse;
use proc_macro_error::{abort_call_site, proc_macro_error};
use syn::parse2;

/// Provides a DSL that conveniently implements the FSM trait.
/// States, Commands and Events are all required to be implemented
/// both as structs and enums.
///
/// An example:
///
/// ```compile_fail
/// #[impl_fsm]
/// impl Fsm<State, Command, Event, EffectHandlers> for MyFsm {
///     state!(Running / entry);
///
///     command!(Idle    => Start => Started => Running);
///     command!(Running => Stop  => Stopped => Idle);
///
///     ignore_command!(Idle    => Stop);
///     ignore_command!(Running => Start);
/// }
/// ```
///
/// The `state!` macro declares state-related attributes. At this time, only entry
/// handlers can be declared. In our example, the macro will ensure that an `on_entry_running`
/// method will be called for `MyFsm`. The developer is then
/// required to implement a method e.g.:
///
/// ```compile_fail
/// fn on_entry_running(_s: &Running, _se: &mut EffectHandlers) {
///     // Do something
/// }
/// ```
///
/// The `command!` macro declares an entire transition using the form:
///
/// ```compile_fail
/// <from-state> => <given-command> [=> <yields-event> []=> <to-state>]]
/// ```
///
/// In our example, for the first transition, multiple methods will be called that the developer must provide e.g.:
///
/// ```compile_fail
/// fn for_idle_start(_s: &Idle, _c: Start, _se: &mut EffectHandlers) -> Option<Started> {
///     // Perform some effect here if required. Effects are performed via the EffectHandler
///     Some(Started)
/// }
///
/// fn on_idle_started(_s: &Idle, _e: &Started) -> Option<Running> {
///     Some(Running)
/// }
/// ```
///
/// The `ignore_command!` macro describes those states and commands that should be ignored given:
///
/// ```compile_fail
/// <from-state> => <given-command>
/// ```
///
/// It is possible to use a wildcard i.e. `_` in place of `<from-state>` and `<to-state>`.
///
/// There are similar macros for events e.g. `event!` and `ignore_event`. For `event!`, the declaration
/// becomes:
///
/// ```compile_fail
/// <from-state> => <given-event> [=> <to-state> [ / action]]
/// ```
///
/// The `/ action` is optional and is used to declare that a side-effect is to be performed.
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
