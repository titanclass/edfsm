pub mod fixtures;
use edfsm::Input;
use fixtures::{Command, Counter, Event, Output};
use machine::{error::Result, machine, Machine};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinSet,
};

async fn producer(sender: Sender<Input<Command, Event>>) -> Result<()> {
    for _ in 1..100 {
        sender.send(Input::Event(Event::Tick)).await?;
    }
    sender.send(Input::Command(Command::Assert(99))).await?;
    Ok(())
}

async fn consumer(mut receiver: Receiver<Output>) -> Result<()> {
    while let Some(o) = receiver.recv().await {
        println!("{o:?}")
    }
    Ok(())
}

fn build() -> impl Machine<M = Counter> {
    let log = Vec::<Event>::default();
    machine::<Counter>().with_event_log(log)
}

#[tokio::test]
async fn connection_test() {
    let (send_o, recv_o) = channel::<Output>(3);
    let (send_o2, recv_o2) = channel::<Output>(3);

    let builder = build().with_output(send_o).merge_output(send_o2);

    let prod_task = producer(builder.input());
    let cons_task = consumer(recv_o);
    let cons_task2 = consumer(recv_o2);

    let mut set = JoinSet::new();
    set.spawn(builder.task());
    set.spawn(cons_task);
    set.spawn(cons_task2);
    set.spawn(prod_task);
    set.join_all().await;
}
