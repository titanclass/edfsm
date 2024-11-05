use edfsm::{Change, Fsm, Input};
use machine::{error::Result, Init, Machine};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinSet,
};

struct Counter;

#[derive(Clone, Debug)]
enum Command {
    Print,
}

#[derive(Clone, Debug)]
enum Event {
    Tick,
    Reset,
}

#[derive(Clone, Debug)]
enum Output {
    Tock,
}

#[derive(Debug, Default)]
struct State {
    count: i32,
}

impl Fsm for Counter {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = Vec<Output>;

    fn for_command(s: &Self::S, c: Self::C, _se: &mut Self::SE) -> Option<Self::E> {
        match c {
            Command::Print => println!("count = {}", s.count),
        }
        None
    }

    fn on_event(s: &mut Self::S, e: &Self::E) -> Option<edfsm::Change> {
        match e {
            Event::Tick => {
                s.count += 1;
                Some(Change::Updated)
            }
            Event::Reset => {
                if s.count == 0 {
                    None
                } else {
                    s.count = 0;
                    Some(Change::Updated)
                }
            }
        }
    }

    fn on_change(s: &Self::S, _e: &Self::E, se: &mut Self::SE, _change: edfsm::Change) {
        if s.count % 10 == 0 {
            se.push(Output::Tock);
        }
    }
}

impl Init<State> for Vec<Output> {
    fn init(&mut self, _: &State) {}
}

async fn producer(sender: Sender<Input<Command, Event>>) -> Result<()> {
    for _ in 1..100 {
        sender.send(Input::Event(Event::Tick)).await?;
    }
    Ok(())
}

async fn consumer(mut receiver: Receiver<Output>) -> Result<()> {
    while let Some(o) = receiver.recv().await {
        println!("{o:?}")
    }
    Ok(())
}

#[tokio::test]
async fn main() {
    let _ = (Command::Print, Event::Reset); // avoid dead code
    let (send_o, recv_o) = channel::<Output>(3);
    let (send_o2, recv_o2) = channel::<Output>(3);
    let log = Vec::<Event>::default();
    let machine = Machine::<Counter>::default();

    let machine = machine
        .connect_event_log(log)
        .connect_output(send_o)
        .connect_output(send_o2);

    let prod_task = producer(machine.input());
    let cons_task = consumer(recv_o);
    let cons_task2 = consumer(recv_o2);

    let mut set = JoinSet::new();
    set.spawn(machine.task());
    set.spawn(cons_task);
    set.spawn(cons_task2);
    set.spawn(prod_task);
    set.join_all().await;
}
