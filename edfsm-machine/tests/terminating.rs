pub mod fixtures;
use edfsm::Input;
use edfsm_machine::{error::Result, machine, Machine};
use fixtures::{Command, Counter, Event, Output};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinSet,
};

async fn producer(sender: Sender<Input<Command, Event>>) -> Result<()> {
    for _ in 1..50 {
        sender.send(Input::Event(Event::Tick)).await?;
    }
    // sender.send(Input::Event(Event::Stop)).await?;
    for _ in 50..100 {
        sender.send(Input::Event(Event::Tick)).await?;
    }
    Ok(())
}

async fn consumer(mut receiver: Receiver<Output>) -> Result<()> {
    let mut count = 0;
    while let Some(o) = receiver.recv().await {
        println!("{o:?}");
        count += 1;
    }
    println!("Count of tock: {count}");
    assert_eq!(count, 4);
    Ok(())
}

#[tokio::test]
async fn terminating_test() {
    let (send_o, recv_o) = channel::<Output>(3);
    let log = Vec::<Event>::default();

    let builder = machine::<Counter>().with_event_log(log).with_output(send_o);

    let prod_task = producer(builder.input());
    let cons_task = consumer(recv_o);

    let mut set = JoinSet::new();
    set.spawn(builder.task());
    set.spawn(cons_task);
    set.spawn(prod_task);
    set.join_all().await;
}
