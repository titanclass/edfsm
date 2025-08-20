pub mod error;
use edfsm_machine::adapter::{Adapter, Feed};
use edfsm_machine::error as mach_error;
pub use error::Result;
use rusqlite::{Connection, OptionalExtension, Params};
use serde::{de::DeserializeOwned, Serialize};
use std::{marker::PhantomData, ops::Range, path::Path, usize};
use tokio::{sync::Mutex, task::block_in_place};

pub trait Persistable
where
    Self: Serialize,
    Self::Key: Serialize,
{
    /// The type of the compaction key.
    type Key;

    /// The compaction key for this event.
    fn compaction_key(&self) -> Self::Key;

    // On receipt of this event it and all preceding buffered events should be persisted.
    // fn checkpoint(&self) -> bool;
}

#[derive(Debug)]
pub struct BackingStore<A> {
    connection: Connection,
    log_range: Range<i64>,
    last_compact_offset: Option<i64>,
    log_low_level: usize,
    log_high_level: usize,
    marker: PhantomData<A>,
}

impl<A> BackingStore<A> {
    pub fn new(
        path: impl AsRef<Path>,
        low_level: usize,
        high_level: usize,
    ) -> Result<BackingStore<A>> {
        // clamp high and low log levels to valid range
        let log_low_level = low_level.max(1).min(usize::MAX - 2);
        let log_high_level = high_level.max(log_low_level + 2);

        // create or open the database
        let connection = Connection::open(path)?;
        Self::create_tables(&connection)?;
        let log_range = Self::query_log_offsets(&connection)?;
        let last_compact_offset = Self::query_compact_offset(&connection)?;

        let store = Self {
            connection,
            log_range,
            last_compact_offset,
            log_low_level,
            log_high_level,
            marker: PhantomData,
        };

        Ok(store)
    }

    const INSERT_LOG: &str = "INSERT INTO log (key, value) VALUES (?, ?)";

    pub fn produce(&mut self, item: A) -> Result<()>
    where
        A: Persistable,
    {
        let key = serde_json::to_string(&item.compaction_key())?;
        let value = serde_json::to_string(&item)?;

        let mut statement = self.connection.prepare_cached(Self::INSERT_LOG)?;
        statement.execute((&*key, &*value))?;
        let offset = self.connection.last_insert_rowid();
        drop(statement);

        if self.log_range.is_empty() {
            self.log_range.start = offset;
        }
        self.log_range.end = offset + 1;

        if self.log_range.end - self.log_range.start > self.log_high_level as i64 {
            self.compact()?;
        }

        Ok(())
    }

    const COMPACT_LOG_TAIL: &str = "INSERT INTO compacted (key, offset, value) 
        SELECT key, offset, value FROM log ORDER by offset WHERE offset > ?";

    const COMPACT_LOG_ALL: &str = "INSERT INTO compacted (key, offset, value) 
        SELECT key, offset, value FROM log ORDER by offset";

    const TRIM_LOG: &str = "DELETE FROM log where offset < ?";

    pub fn compact(&mut self) -> Result<()> {
        if !self.log_range.is_empty() {
            let last_log_offset = self.log_range.end - 1;

            if let Some(offset) = self.last_compact_offset {
                if last_log_offset > offset {
                    let mut statement = self.connection.prepare_cached(Self::COMPACT_LOG_TAIL)?;
                    statement.execute((offset,))?;
                    self.last_compact_offset = Some(last_log_offset);
                }
            } else {
                let mut statement = self.connection.prepare_cached(Self::COMPACT_LOG_ALL)?;
                statement.execute(())?;
                self.last_compact_offset = Some(last_log_offset);
            }

            if self.log_range.end - self.log_range.start > self.log_high_level as i64 {
                let preserve = self.log_range.end - self.log_low_level as i64;
                let mut statement = self.connection.prepare_cached(Self::TRIM_LOG)?;
                statement.execute((preserve,))?;
                self.log_range.start = preserve;
            }
        }
        Ok(())
    }

    const SELECT_LOG: &str = "SELECT value FROM log ORDER BY offset";
    const SELECT_COMPACT_ALL: &str = "SELECT value FROM compact ORDER BY offset";
    const SELECT_COMPACT_TAIL: &str = "SELECT value FROM compact ORDER BY offset WHERE offset > ?";

    fn query_events<P>(&self, sql: &str, params: P, values: &mut Vec<A>) -> Result<()>
    where
        A: DeserializeOwned,
        P: Params,
    {
        let mut statement: rusqlite::CachedStatement<'_> = self.connection.prepare_cached(sql)?;
        let mut rows = statement.query(params)?;

        while let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            let item: A = serde_json::from_str(&*text)?;
            values.push(item);
        }
        Ok(())
    }

    pub fn history(&mut self) -> Result<Vec<A>>
    where
        A: DeserializeOwned,
    {
        let mut values: Vec<A> = Vec::new();

        if self.log_range.is_empty() {
            self.query_events(Self::SELECT_COMPACT_ALL, (), &mut values)?;
        } else {
            let breakpoint = self.log_range.end - 1;
            self.query_events(Self::SELECT_LOG, (), &mut values)?;
            self.query_events(Self::SELECT_COMPACT_TAIL, (breakpoint,), &mut values)?;
        }

        Ok(values)
    }

    const CREATE_LOG: &str = "CREATE TABLE IF NOT EXISTS log (
        offset INTEGER PRIMARY KEY,
        key TEXT,
        value TEXT

    )";

    const CREATE_COMPACT: &str = "CREATE TABLE IF NOT EXISTS compacted (
        key TEXT PRIMARY KEY ON CONFLICT REPLACE,
        offset INTEGER,
        value TEXT

    )";

    fn create_tables(connection: &Connection) -> Result<()> {
        connection.execute(Self::CREATE_LOG, ())?;
        connection.execute(Self::CREATE_COMPACT, ())?;
        Ok(())
    }

    const QUERY_LOG_OFFSETS: &str = "SELECT MIN(offset), MAX(offset) FROM log";

    fn query_log_offsets(connection: &Connection) -> Result<Range<i64>> {
        let mut statement = connection.prepare_cached(Self::QUERY_LOG_OFFSETS)?;
        let values = statement
            .query_row((), |row| {
                let start: i64 = row.get(0)?;
                let last: i64 = row.get(1)?;
                let end = last + 1;
                Ok(Range { start, end })
            })
            .optional()?
            .unwrap_or_default();
        Ok(values)
    }

    const QUERY_COMPACT_OFFSET: &str = "SELECT MAX(offset) FROM compact";

    fn query_compact_offset(connection: &Connection) -> Result<Option<i64>> {
        let mut statement = connection.prepare_cached(Self::QUERY_COMPACT_OFFSET)?;
        let offset = statement.query_row((), |row| row.get(0)).optional()?;
        Ok(offset)
    }
}

#[derive(Debug)]
pub struct AsyncBackingStore<A>(Mutex<BackingStore<A>>);

impl<A> AsyncBackingStore<A> {
    pub fn new(store: BackingStore<A>) -> Self {
        Self(Mutex::new(store))
    }
}

impl<A> Feed for AsyncBackingStore<A>
where
    A: DeserializeOwned + Send + Sync + 'static,
{
    type Item = A;

    async fn feed(&self, sink: &mut impl Adapter<Item = Self::Item>) -> mach_error::Result<()> {
        let mut store = self.0.lock().await;
        let values = block_in_place(|| {
            store.compact()?;
            store.history()
        })?;
        for item in values {
            sink.notify(item).await;
        }
        Ok(())
    }
}

impl<A> Adapter for AsyncBackingStore<A>
where
    A: Send + Sync + Persistable,
{
    type Item = A;

    async fn notify(&mut self, item: Self::Item) {
        let mut store = self.0.lock().await;
        let _ = block_in_place(|| store.produce(item));
    }
}
