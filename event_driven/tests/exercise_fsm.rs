// Declare our state, commands and events

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

// Declare the FSM itself

struct MyFsm {}

#[impl_fsm]
impl Fsm<State, Input, Output, ()> for MyFsm {
    state!(B / entry);
    state!(B / exit);

    transition!(A => I0 => O0 => B);
    transition!(B => I1 => O1 => A);
    transition!(B => I2 => O2);
    transition!(B => I3);

    transition!(_ => I1 => O1 => A);
    transition!(_ => I2 => O2);
    transition!(_ => I3);
}

impl MyFsm {
    fn for_a_i0_o0(_s: &A, _c: I0, _se: &mut ()) -> Option<O0> {
        Some(O0)
    }

    fn for_a_o0_b(_s: &A, _e: &O0) -> Option<B> {
        Some(B)
    }

    fn for_b_i1_o1(_s: &B, _c: I1, _se: &mut ()) -> Option<O1> {
        Some(O1)
    }

    fn for_b_o1_a(_s: &B, _e: &O1) -> Option<A> {
        Some(A)
    }

    fn for_b_i2_o2(_s: &B, _c: I2, _se: &mut ()) -> Option<O2> {
        Some(O2)
    }

    fn for_b_i3(_s: &B, _c: I3, _se: &mut ()) {}

    fn for_any_i1_o1(_s: &State, _c: I1, _se: &mut ()) -> Option<O1> {
        Some(O1)
    }

    fn for_any_o1_a(_s: &State, _e: &O1) -> Option<A> {
        Some(A)
    }

    fn for_any_i2_o2(_s: &State, _c: I2, _se: &mut ()) -> Option<O2> {
        Some(O2)
    }

    fn for_any_i3(_s: &State, _c: I3, _se: &mut ()) {}

    fn on_entry_b(_to_s: &B, _se: &mut ()) {}

    fn on_exit_b(_old_s: &B, _se: &mut ()) {}
}

#[test]
fn main() {
    let _ = MyFsm::step(&State::A(A), Input::I0(I0), &mut ());
    let _ = MyFsm::step(&State::B(B), Input::I1(I1), &mut ());
    let _ = MyFsm::step(&State::B(B), Input::I2(I2), &mut ());
    let _ = MyFsm::step(&State::B(B), Input::I3(I3), &mut ());
}
