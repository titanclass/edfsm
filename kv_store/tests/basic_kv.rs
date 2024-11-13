pub mod fixtures;
use edfsm::Input;
use fixtures::{Counter, Event, Output, State};
use kv_store::{requester, Keyed, KvStore, Path, Query};
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
    let mut tock_count = 0;
    while let Some(o) = receiver.recv().await {
        println!("{o:?}");
        tock_count += 1;
    }
    assert_eq!(tock_count, 9);
    Ok(())
}

async fn asker(sender: Sender<Input<Query<State>, Keyed<Event>>>) -> Result<()> {
    let mut r = requester(sender);

    let n = r.get(Path::root(), |s| s.unwrap().count).await?;
    println!("The root element state is {n}");

    let n = r.get_all(|ss| ss.fold(0, |t, (_p, s)| t + s.count)).await?;
    println!("The sum of all element states {n}");
    Ok(())
}

#[tokio::test]
async fn basic_kv_test() {
    let (send_o, recv_o) = channel::<Keyed<Output>>(3);

    let machine = Machine::<KvStore<Counter>>::default().connect_output(send_o);
    let prod_task = producer(machine.input());
    let cons_task = consumer(recv_o);
    let ask_task = asker(machine.input());

    let mut set = JoinSet::new();
    set.spawn(machine.task());
    set.spawn(cons_task);
    set.spawn(prod_task);
    set.spawn(ask_task);
    set.join_all().await;
}
