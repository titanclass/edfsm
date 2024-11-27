pub mod fixtures;
use edfsm::Input;
use edfsm_machine::{error::Result, machine, Machine};
use fixtures::{Command, Counter, Event};
use streambed_codec::{Cbor, CommitLogExt};
use streambed_logged::FileLog;
use tokio::{spawn, sync::mpsc::Sender, task::JoinSet};

const TEST_DATA: &str = "test_data";
const TOPIC: &str = "event_series";

async fn producer(sender: Sender<Input<Command, Event>>) -> Result<()> {
    for _ in 1..100 {
        sender.send(Input::Event(Event::Tick)).await?;
    }
    sender.send(Input::Command(Command::Assert(99))).await?;
    Ok(())
}

async fn phase_1() {
    let topic_file = [TEST_DATA, TOPIC].join("/");
    let _ = std::fs::remove_file(&topic_file);
    let _ = std::fs::create_dir(TEST_DATA);

    let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, Cbor);
    let machine = machine::<Counter>().with_event_log(log);
    let prod_task = producer(machine.input());

    let mut set = JoinSet::new();
    set.spawn(machine.task());
    set.spawn(prod_task);
    set.join_all().await;
}

async fn phase_2() {
    let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, Cbor);
    let machine = machine::<Counter>().with_event_log(log);
    let sender = machine.input();
    let handle = spawn(machine.task());
    sender
        .send(Input::Command(Command::Assert(99)))
        .await
        .unwrap();
    drop(sender);
    handle.await.unwrap().unwrap();
}

#[tokio::test]
async fn streambed_test() {
    phase_1().await;
    phase_2().await;
}
