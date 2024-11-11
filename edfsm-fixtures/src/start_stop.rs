// Declare our state, commands and events

use edfsm::{impl_fsm, Fsm};

pub struct Idle;
pub struct Running;
pub enum State {
    Idle(Idle),
    Running(Running),
}

pub struct Start;
pub struct Stop;
pub enum Command {
    Start(Start),
    Stop(Stop),
}

pub struct Started;
pub struct Stopped;
pub enum Event {
    Started(Started),
    Stopped(Stopped),
}

// Declare an object to handle effects as we step through the FSM

pub struct EffectHandlers {
    started: u32,
    stopped: u32,
    transitioned_stopped_to_started: u32,
}

impl EffectHandlers {
    pub fn start_something(&mut self) {
        self.started += 1;
    }

    pub fn stop_something(&mut self) {
        self.stopped += 1;
    }

    pub fn enter_running(&mut self) {
        self.transitioned_stopped_to_started += 1;
    }
}

// Declare the FSM itself

pub struct StartStopFsm;

#[impl_fsm]
impl Fsm for StartStopFsm {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = EffectHandlers;

    state!(Running / entry);

    command!(Idle    => Start => Started => Running);
    command!(Running => Stop  => Stopped => Idle);

    ignore_command!(Idle    => Stop);
    ignore_command!(Running => Start);
}

impl StartStopFsm {
    fn on_entry_running(_to_s: &Running, se: &mut EffectHandlers) {
        se.enter_running()
    }

    fn for_running_stop(_s: &Running, _c: Stop, se: &mut EffectHandlers) -> Option<Stopped> {
        se.stop_something();
        Some(Stopped)
    }

    fn on_running_stopped(_s: &Running, _e: &Stopped) -> Option<Idle> {
        Some(Idle)
    }

    fn for_idle_start(_s: &Idle, _c: Start, se: &mut EffectHandlers) -> Option<Started> {
        se.start_something();
        Some(Started)
    }

    fn on_idle_started(_s: &Idle, _e: &Started) -> Option<Running> {
        Some(Running)
    }
}
