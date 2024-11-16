use async_stream::stream;
use futures_util::{Stream, StreamExt};
use rand::thread_rng;
use serde::{de::DeserializeOwned, Serialize};
use std::{marker::PhantomData, pin::Pin, vec::Vec};
use streambed::{
    commit_log::{Offset, ProducerRecord, Subscription, Topic},
    decrypt_buf_with_secret, encrypt_struct_with_secret, get_secret_value,
    secret_store::SecretStore,
};

pub use streambed::commit_log::{CommitLog, ProducerError};

/// Wraps a `CommitLog` and specializes it for a specific payload type.
/// This adds the type, topic and the encoding and encryption scheme.
#[derive(Debug)]
pub struct LogAdapter<L, C, A> {
    commit_log: L,
    codec: C,
    topic: Topic,
    group: String,
    marker: PhantomData<A>,
}

/// Provides a method on `CommitLog` to specialize it for a payload type.
pub trait CommitLogExt
where
    Self: CommitLog + Sized,
{
    /// Specialize this commit log for items of type `A`
    /// The topic and group names are given and a `Codec`
    /// for encoding and decoding values of type `A`.
    fn adapt<A>(
        self,
        topic: impl Into<Topic>,
        group: &str,
        codec: impl Codec<A>,
    ) -> LogAdapter<Self, impl Codec<A>, A> {
        LogAdapter {
            commit_log: self,
            codec,
            topic: topic.into(),
            group: group.to_string(),
            marker: PhantomData,
        }
    }
}

impl<L> CommitLogExt for L where L: CommitLog {}

impl<L, C, A> LogAdapter<L, C, A>
where
    L: CommitLog,
    C: Codec<A>,
    A: 'static,
{
    /// Send one item to the underlying commit log.
    pub async fn produce(&self, item: A) -> Result<Offset, ProducerError> {
        let topic = self.topic.clone();

        if let Some(value) = self.codec.encode(item) {
            self.commit_log
                .produce(ProducerRecord {
                    topic,
                    headers: Vec::new(),
                    timestamp: None,
                    key: 0,
                    value,
                    partition: 0,
                })
                .await
                .map(|r| r.offset)
        } else {
            Err(ProducerError::CannotProduce)
        }
    }

    /// Return an async stream of items representing the
    /// history up to the time of the call.
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
                while let Some(mut r) = records.next().await {
                    if r.offset <= last_offset {
                        if let Some(item) = self.codec.decode(&mut r.value) {
                            yield item;
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

/// A trait for codecs for a specic type `A`, usually an event type stored on a CommitLog.
pub trait Codec<A> {
    /// Encode a value.
    fn encode(&self, item: A) -> Option<Vec<u8>>;
    /// Decode a value.
    fn decode(&self, bytes: &mut [u8]) -> Option<A>;
}

/// A `Codec` for encripted CBOR
#[derive(Debug)]
pub struct CborEncrypted {
    secret: String,
}

impl CborEncrypted {
    /// Create an encrypted CBOR codec with the given secret store.
    pub async fn new<S>(secret_store: &S, secret_path: &str) -> Option<Self>
    where
        S: SecretStore,
    {
        Some(Self {
            secret: get_secret_value(secret_store, secret_path).await?,
        })
    }
}

impl<A> Codec<A> for CborEncrypted
where
    A: Serialize + DeserializeOwned + Send,
{
    fn encode(&self, item: A) -> Option<Vec<u8>> {
        let serialize = |item: &A| {
            let mut buf = Vec::new();
            ciborium::ser::into_writer(item, &mut buf).map(|_| buf)
        };
        encrypt_struct_with_secret(self.secret.clone(), serialize, thread_rng, &item)
    }

    fn decode(&self, bytes: &mut [u8]) -> Option<A> {
        decrypt_buf_with_secret(self.secret.clone(), bytes, |b| {
            ciborium::de::from_reader::<A, _>(b)
        })
    }
}

/// A `Codec` for CBOR
#[derive(Debug)]
pub struct Cbor;

impl<A> Codec<A> for Cbor
where
    A: Serialize + DeserializeOwned + Send,
{
    fn encode(&self, item: A) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&item, &mut buf).ok()?;
        Some(buf)
    }

    fn decode(&self, bytes: &mut [u8]) -> Option<A> {
        ciborium::de::from_reader::<A, &[u8]>(bytes).ok()
    }
}

#[cfg(test)]
mod test {

    use crate::{Cbor, CborEncrypted, CommitLogExt};
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

    fn fixture_store() -> FileSecretStore {
        todo!()
    }

    fn fixture_data() -> impl Iterator<Item = Event> {
        (1..100).map(Event::Num)
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
        let codec = CborEncrypted::new(&fixture_store(), "secret_path")
            .await
            .unwrap();
        let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, "group", codec);
        let mut history = log.history().await;
        while let Some(event) = history.next().await {
            println!("{event:?}")
        }
    }

    #[tokio::test]
    #[ignore]
    async fn cbor_encrypted_produce() {
        let codec = CborEncrypted::new(&fixture_store(), "secret_path")
            .await
            .unwrap();
        let log = FileLog::new(TEST_DATA).adapt::<Event>(TOPIC, "group", codec);
        for i in 1..100 {
            let _ = log.produce(Event::Num(i)).await;
        }
    }
}
