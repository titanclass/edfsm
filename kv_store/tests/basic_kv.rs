pub mod fixtures;
use edfsm::Input;
use fixtures::{Counter, Event, Output, State};
use kv_store::{requester, Keyed, KvStore, Path, Query};
use machine::{error::Result, Machine};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinSet,
};

async fn producer(
    sender: Sender<Input<Query<State, Event>, Keyed<Event>>>,
    count: i32,
) -> Result<()> {
    for _ in 0..count {
        sender
            .send(Input::Event(Keyed {
                key: Path::root(),
                item: Event::Tick,
            }))
            .await?;
    }
    Ok(())
}

async fn consumer(mut receiver: Receiver<Keyed<Output>>, expect: i32) -> Result<()> {
    let mut tock_count = 0;
    while let Some(o) = receiver.recv().await {
        println!("{o:?}");
        tock_count += 1;
    }
    assert_eq!(tock_count, expect);
    Ok(())
}

async fn asker(sender: Sender<Input<Query<State, Event>, Keyed<Event>>>) -> Result<()> {
    let mut r = requester(sender);

    if let Some(n) = r.get(Path::root(), |s| s.map(|s| s.count)).await? {
        println!("The root element state is {n}");
    } else {
        println!("Root path not present")
    }

    let n = r.get_all(|ss| ss.fold(0, |t, (_p, s)| t + s.count)).await?;
    println!("The sum of all element states {n}");
    Ok(())
}

#[tokio::test]
async fn basic_kv_test() {
    let (send_o, recv_o) = channel::<Keyed<Output>>(3);

    let machine = Machine::<KvStore<Counter>>::default().connect_output(send_o);
    let prod_task = producer(machine.input(), 99);
    let cons_task = consumer(recv_o, 9);
    let ask_task = asker(machine.input());

    let mut set = JoinSet::new();
    set.spawn(machine.task());
    set.spawn(cons_task);
    set.spawn(prod_task);
    set.spawn(ask_task);
    set.join_all().await;
}

#[tokio::test]
async fn empty_kv_test() {
    let (send_o, recv_o) = channel::<Keyed<Output>>(3);

    let machine = Machine::<KvStore<Counter>>::default().connect_output(send_o);
    let cons_task = consumer(recv_o, 0);
    let ask_task = asker(machine.input());

    let mut set = JoinSet::new();
    set.spawn(machine.task());
    set.spawn(cons_task);
    set.spawn(ask_task);
    set.join_all().await;
}
