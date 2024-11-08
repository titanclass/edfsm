use async_stream::stream;
use futures_util::{Stream, StreamExt};
use rand::thread_rng;
use serde::{de::DeserializeOwned, Serialize};
use std::{future::Future, marker::PhantomData, pin::Pin, vec::Vec};
use streambed::{
    commit_log::{Offset, ProducerRecord, Subscription, Topic},
    decrypt_buf, encrypt_struct_with_secret, get_secret_value,
    secret_store::SecretStore,
};

pub use streambed::commit_log::{CommitLog, ProducerError};

/// Provides the compaction key for an event.
pub trait CompactionKey {
    fn compaction_key(&self) -> u64;
}

/// Wraps a `CommitLog` and specializes it for a specific event type.
/// This adds the event type, topic and the encoding and encryption scheme.
#[derive(Debug)]
pub struct LogAdapter<L, C, A> {
    commit_log: L,
    codec: C,
    topic: Topic,
    group: String,
    marker: PhantomData<A>,
}

/// Provides a method on `CommitLog` to specialize it for an event type.
pub trait CommitLogExt
where
    Self: CommitLog + Sized,
{
    /// Specialize this commit log for events of type `A`
    /// The topic and group names are given and a `Codec`
    /// for encoding and decoding values of type `A`.
    fn adapt<A>(
        self,
        topic: &str,
        group: &str,
        codec: impl Codec<A>,
    ) -> LogAdapter<Self, impl Codec<A>, A> {
        LogAdapter {
            commit_log: self,
            codec,
            topic: topic.into(),
            group: group.into(),
            marker: PhantomData,
        }
    }
}

impl<L> CommitLogExt for L where L: CommitLog {}

impl<L, C, A> LogAdapter<L, C, A>
where
    L: CommitLog,
    C: Codec<A>,
    A: CompactionKey + Clone + 'static,
{
    /// Send one event to the underlying commit log.
    pub async fn produce(&self, item: A) -> Result<Offset, ProducerError> {
        let key = item.compaction_key();
        let topic = self.topic.clone();

        if let Some(value) = self.codec.encode(item).await {
            self.commit_log
                .produce(ProducerRecord {
                    topic,
                    headers: Vec::new(),
                    timestamp: None,
                    key,
                    value,
                    partition: 0,
                })
                .await
                .map(|r| r.offset)
        } else {
            Err(ProducerError::CannotProduce)
        }
    }

    /// Return an async stream of events representing the
    /// event history up to the time of the call.
    #[allow(clippy::needless_lifetimes)]
    pub async fn history<'a>(&'a self) -> Pin<Box<impl Stream<Item = A> + 'a>> {
        let last_offset = self
            .commit_log
            .offsets(self.topic.clone(), 0)
            .await
            .map(|lo| lo.end_offset);
        let subscriptions = Vec::from([Subscription {
            topic: self.topic.clone(),
        }]);

        let mut records =
            self.commit_log
                .scoped_subscribe(&self.group, Vec::new(), subscriptions, None);

        Box::pin(stream! {
            if let Some(last_offset) = last_offset {
                while let Some(r) = records.next().await {
                    if r.offset <= last_offset {
                        if let Some(event) = self.codec.decode(r.value).await {
                            yield event;
                        }
                        if r.offset == last_offset {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        })
    }
}

/// A `Codec` for encripted CBOR
#[derive(Debug)]
pub struct CborEncrypted<S> {
    secret_store: S,
    secret_path: String,
}

impl<S> CborEncrypted<S>
where
    S: SecretStore,
{
    /// Create an encrypted CBOR codec with the given secret store.
    pub fn new(secret_store: S, path: &str) -> Self {
        Self {
            secret_store,
            secret_path: path.into(),
        }
    }
}

/// A trait for asyncronous codecs.
pub trait Codec<A> {
    fn encode(&self, item: A) -> impl Future<Output = Option<Vec<u8>>> + Send;
    fn decode(&self, bytes: Vec<u8>) -> impl Future<Output = Option<A>> + Send;
}

impl<S, A> Codec<A> for CborEncrypted<S>
where
    S: SecretStore,
    A: Serialize + DeserializeOwned + Send,
{
    async fn encode(&self, item: A) -> Option<Vec<u8>> {
        let secret_value = get_secret_value(&self.secret_store, &self.secret_path).await?;
        let serialize = |item: &A| {
            let mut buf = Vec::new();
            ciborium::ser::into_writer(item, &mut buf).map(|_| buf)
        };
        encrypt_struct_with_secret(secret_value, serialize, thread_rng, &item)
    }

    async fn decode(&self, mut bytes: Vec<u8>) -> Option<A> {
        decrypt_buf(&self.secret_store, &self.secret_path, &mut bytes, |b| {
            ciborium::de::from_reader::<A, _>(b)
        })
        .await
    }
}

/// A `Codec` for CBOR
#[derive(Debug)]
pub struct Cbor;

impl<A> Codec<A> for Cbor
where
    A: Serialize + DeserializeOwned + Send,
{
    async fn encode(&self, item: A) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&item, &mut buf).ok()?;
        Some(buf)
    }

    async fn decode(&self, bytes: Vec<u8>) -> Option<A> {
        ciborium::de::from_reader::<A, &[u8]>(&bytes).ok()
    }
}

#[cfg(test)]
mod test {

    use crate::{Cbor, CborEncrypted, CommitLogExt, CompactionKey};
    use futures_util::StreamExt;
    use serde::{Deserialize, Serialize};
    use std::path::Path;
    use streambed_confidant::FileSecretStore;
    use streambed_logged::FileLog;
    use tokio::task::yield_now;

    // use std::time::Duration;
    // use tokio::time::sleep;

    const TEST_DATA: &str = "test_data";
    const TOPIC: &str = "event_series";

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub enum Event {
        Num(u32),
    }

    impl CompactionKey for Event {
        fn compaction_key(&self) -> u64 {
            0
        }
    }

    fn fixture_store() -> FileSecretStore {
        todo!()
    }

    fn fixture_data() -> impl Iterator<Item = Event> {
        (1..100).into_iter().map(Event::Num)
    }

    #[tokio::test]
    async fn cbor_history() {
        cbor_produce().await;
        // sleep(Duration::from_secs(1)).await;
        let mut data = fixture_data();
        let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, "group", Cbor);
        let mut history = log.history().await;
        while let Some(event) = history.next().await {
            println!("{event:?}");
            assert_eq!(event, data.next().unwrap());
        }
        assert!(data.next().is_none());
    }

    async fn cbor_produce() {
        let topic_file = [TEST_DATA, TOPIC].join("/");
        let _ = std::fs::remove_file(&topic_file);
        let _ = std::fs::create_dir(TEST_DATA);
        let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, "group", Cbor);
        for e in fixture_data() {
            log.produce(e).await.expect("failed to produce a log entry");
        }
        assert!(Path::new(&topic_file).exists());
        drop(log);
        yield_now().await;
    }

    #[tokio::test]
    async fn cbor_produce_test() {
        cbor_produce().await;
    }

    #[tokio::test]
    #[ignore]
    async fn cbor_encrypted_history() {
        let codec = CborEncrypted::new(fixture_store(), "secret_path");
        let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, "group", codec);
        let mut history = log.history().await;
        while let Some(event) = history.next().await {
            println!("{event:?}")
        }
    }

    #[tokio::test]
    #[ignore]
    async fn cbor_encrypted_produce() {
        let codec = CborEncrypted::new(fixture_store(), "secret_path");
        let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, "group", codec);
        for i in 1..100 {
            let _ = log.produce(Event::Num(i)).await;
        }
    }
}
