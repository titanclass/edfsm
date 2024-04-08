// Declare our state, commands and events

use std::marker::PhantomData;

use edfsm::{impl_fsm, Fsm};

struct A;
struct B;
enum State {
    A(A),
    B(B),
}

struct I0;
struct I1;
struct I2;
struct I3;
enum Command {
    I0(I0),
    I1(I1),
    I2(I2),
    I3(I3),
}

struct O0;
struct O1;
struct O2;
enum Event {
    O0(O0),
    O1(O1),
    O2(O2),
}

// This next bit of code illustrates how a trait can be used to declare
// effect handlers. We leverage Dynamically Sized Types to do this.
// For more information: https://doc.rust-lang.org/nomicon/exotic-sizes.html#:~:text=Rust%20supports%20Dynamically%20Sized%20Types,DSTs%20are%20not%20normal%20types.

trait EffectHandlers {
    fn say_hi(&self);
}

struct EffectHandlerBox<SE: EffectHandlers + ?Sized>(SE);

// Declare the FSM itself

struct MyFsm<SE: EffectHandlers> {
    pub phantom: PhantomData<SE>,
}

#[impl_fsm]
impl<SE: EffectHandlers> Fsm for MyFsm<SE> {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = EffectHandlerBox<SE>;

    state!(B / entry);

    command!(A => I0 => O0 => B);
    command!(B => I1 => O1 => A | B);
    command!(B => I2 => O2);
    event!(  B       => O2);
    command!(B => I3);

    command!(_ => I1 => O1 => A);
    command!(_ => I2 => O2);
    command!(_ => I3);

    ignore_event!(  A => O2);
    ignore_command!(B => I0);
    ignore_event!(  B => O0);
}

impl<SE: EffectHandlers> MyFsm<SE> {
    fn for_a_i0(_s: &A, _c: I0, se: &mut EffectHandlerBox<SE>) -> Option<O0> {
        se.0.say_hi();
        Some(O0)
    }

    fn on_a_o0(_s: &A, _e: &O0) -> Option<B> {
        Some(B)
    }

    fn on_entry_b(_to_s: &B, _se: &mut EffectHandlerBox<SE>) {}

    fn for_b_i1(_s: &B, _c: I1, _se: &mut EffectHandlerBox<SE>) -> Option<O1> {
        Some(O1)
    }

    fn on_b_o1(_s: &B, _e: &O1) -> Option<State> {
        Some(State::A(A))
    }

    fn for_b_i2(_s: &B, _c: I2, _se: &mut EffectHandlerBox<SE>) -> Option<O2> {
        Some(O2)
    }

    fn on_b_o2(_s: &B, _e: &O2) {}

    fn on_change_b_o2(_s: &B, _e: &O2, _se: &mut EffectHandlerBox<SE>) {}

    fn for_b_i3(_s: &B, _c: I3, _se: &mut EffectHandlerBox<SE>) {}

    fn for_any_i1(_s: &State, _c: I1, _se: &mut EffectHandlerBox<SE>) -> Option<O1> {
        Some(O1)
    }

    fn on_any_o1(_s: &State, _e: &O1) -> Option<A> {
        Some(A)
    }

    fn for_any_i2(_s: &State, _c: I2, _se: &mut EffectHandlerBox<SE>) -> Option<O2> {
        Some(O2)
    }

    fn on_any_o2(_s: &mut State, _e: &O2) {}

    fn for_any_i3(_s: &State, _c: I3, _se: &mut EffectHandlerBox<SE>) {}
}

#[test]
fn main() {
    struct MyEffectHandlers;
    impl EffectHandlers for MyEffectHandlers {
        fn say_hi(&self) {
            println!("hi!");
        }
    }
    let mut se = EffectHandlerBox(MyEffectHandlers);

    let _ = MyFsm::step(&mut State::A(A), Command::I0(I0), &mut se);
    let _ = MyFsm::step(&mut State::B(B), Command::I1(I1), &mut se);
    let _ = MyFsm::step(&mut State::B(B), Command::I2(I2), &mut se);
    let _ = MyFsm::step(&mut State::B(B), Command::I3(I3), &mut se);
}
