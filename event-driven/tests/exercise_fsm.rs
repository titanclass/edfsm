// Declare our state, commands and events

use std::{future::Future, marker::PhantomData};

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
enum Input {
    I0(I0),
    I1(I1),
    I2(I2),
    I3(I3),
}

struct O0;
struct O1;
struct O2;
enum Output {
    O0(O0),
    O1(O1),
    O2(O2),
}

// This next bit of code illustrates how a trait can be used to declare
// effect handlers. We leverage Dynamically Sized Types to do this.
// For more information: https://doc.rust-lang.org/nomicon/exotic-sizes.html#:~:text=Rust%20supports%20Dynamically%20Sized%20Types,DSTs%20are%20not%20normal%20types.

trait EffectHandlers {
    fn say_hi(&self) -> impl Future<Output = ()>;
}

struct EffectHandlerBox<SE: EffectHandlers + ?Sized>(SE);

// Declare the FSM itself

struct MyFsm<SE: EffectHandlers> {
    pub phantom: PhantomData<SE>,
}

#[impl_fsm]
impl<SE: EffectHandlers> Fsm for MyFsm<SE> {
    type S = State;
    type C = Input;
    type E = Output;
    type SE = EffectHandlerBox<SE>;

    state!(B / entry);

    transition!(A => I0 => O0 => B);
    transition!(B => I1 => O1 => A | B);
    transition!(B => I2 => O2);
    transition!(B => I3);

    transition!(_ => I1 => O1 => A);
    transition!(_ => I2 => O2);
    transition!(_ => I3);

    ignore!(B => I0);
}

impl<SE: EffectHandlers> MyFsm<SE> {
    async fn for_a_i0(_s: &A, _c: I0, se: &mut EffectHandlerBox<SE>) -> Option<O0> {
        se.0.say_hi().await;
        Some(O0)
    }

    fn on_a_o0(_s: &A, _e: &O0) -> Option<B> {
        Some(B)
    }

    async fn on_entry_b(_to_s: &B, _se: &mut EffectHandlerBox<SE>) {}

    async fn for_b_i1(_s: &B, _c: I1, _se: &mut EffectHandlerBox<SE>) -> Option<O1> {
        Some(O1)
    }

    fn on_b_o1(_s: &B, _e: &O1) -> Option<State> {
        Some(State::A(A))
    }

    async fn for_b_i2(_s: &B, _c: I2, _se: &mut EffectHandlerBox<SE>) -> Option<O2> {
        Some(O2)
    }

    fn on_b_o2(_s: &B, _e: &O2) {}

    async fn for_b_i3(_s: &B, _c: I3, _se: &mut EffectHandlerBox<SE>) {}

    async fn for_any_i1(_s: &State, _c: I1, _se: &mut EffectHandlerBox<SE>) -> Option<O1> {
        Some(O1)
    }

    fn on_any_o1(_s: &State, _e: &O1) -> Option<A> {
        Some(A)
    }

    async fn for_any_i2(_s: &State, _c: I2, _se: &mut EffectHandlerBox<SE>) -> Option<O2> {
        Some(O2)
    }

    fn on_any_o2(_s: &mut State, _e: &O2) {}

    async fn for_any_i3(_s: &State, _c: I3, _se: &mut EffectHandlerBox<SE>) {}
}

#[tokio::test]
async fn main() {
    struct MyEffectHandlers;
    impl EffectHandlers for MyEffectHandlers {
        async fn say_hi(&self) {
            println!("hi!");
        }
    }
    let mut se = EffectHandlerBox(MyEffectHandlers);

    let _ = MyFsm::step(&mut State::A(A), Input::I0(I0), &mut se).await;
    let _ = MyFsm::step(&mut State::B(B), Input::I1(I1), &mut se).await;
    let _ = MyFsm::step(&mut State::B(B), Input::I2(I2), &mut se).await;
    let _ = MyFsm::step(&mut State::B(B), Input::I3(I3), &mut se).await;
}
