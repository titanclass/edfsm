use edfsm::Input;
use edfsm_fixtures::counter::{Counter, Event, Output, State};
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

async fn consumer(mut receiver: Receiver<Output>) -> Result<()> {
    while let Some(o) = receiver.recv().await {
        println!("{o:?}")
    }
    Ok(())
}

#[tokio::test]
async fn basic_kv_test() {
    let (send_o, recv_o) = channel::<Output>(3);
    let (send_o2, recv_o2) = channel::<Output>(3);
    let log = Vec::<Keyed<Event>>::default();

    let machine = Machine::<KvStore<Counter>>::default()
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
