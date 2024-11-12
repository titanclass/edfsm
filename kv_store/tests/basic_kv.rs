pub mod fixtures;
use edfsm::Input;
use fixtures::{Counter, Event, Output, State};
use kv_store::{Keyed, KvStore, Path, Query};
use machine::{error::Result, Machine};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinSet,
};

async fn producer(sender: Sender<Input<Query<State>, Keyed<Event>>>) -> Result<()> {
    for _ in 1..100 {
        sender
            .send(Input::Event(Keyed {
                key: Path::root(),
                item: Event::Tick,
            }))
            .await?;
    }
    Ok(())
}

async fn consumer(mut receiver: Receiver<Keyed<Output>>) -> Result<()> {
    while let Some(o) = receiver.recv().await {
        println!("{o:?}")
    }
    Ok(())
}

#[tokio::test]
async fn basic_kv_test() {
    let (send_o, recv_o) = channel::<Keyed<Output>>(3);

    let machine = Machine::<KvStore<Counter>>::default().connect_output(send_o);
    let prod_task = producer(machine.input());
    let cons_task = consumer(recv_o);

    let mut set = JoinSet::new();
    set.spawn(machine.task());
    set.spawn(cons_task);
    set.spawn(prod_task);
    set.join_all().await;
}
