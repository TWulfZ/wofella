use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::error::StoreError;

/// Architecture §7 and spec 003: enough readers for UI queries next to a running job.
const READ_POOL_SIZE: usize = 4;
/// Bounded so a stalled writer applies backpressure instead of queueing unbounded work.
const WRITE_QUEUE_CAPACITY: usize = 256;
const BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

/// Durability class of a database file (architecture §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbKind {
    /// Irreplaceable ledger: every commit is fsynced.
    User,
    /// Disposable derived data: WAL + NORMAL may lose the last commits on power loss, which a
    /// rebuild recovers.
    Cache,
}

impl DbKind {
    fn synchronous(self) -> &'static str {
        match self {
            Self::User => "FULL",
            Self::Cache => "NORMAL",
        }
    }
}

/// A write transaction on the single writer thread. Committed when the job returns `Ok`.
pub struct Tx<'a>(pub(crate) rusqlite::Transaction<'a>);

impl Tx<'_> {
    /// Read-side view of the same transaction, so reads inside a write see its own changes.
    pub fn conn(&self) -> Conn<'_> {
        Conn(&self.0)
    }
}

/// A read-only connection from the pool, or the read view of a write transaction.
#[derive(Clone, Copy)]
pub struct Conn<'a>(pub(crate) &'a Connection);

type WriteJob = Box<dyn FnOnce(&mut Connection) + Send>;

struct Writer {
    queue: Mutex<Option<Sender<WriteJob>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Writer {
    fn stop(&self) {
        // Closing the queue ends the thread's loop once in-flight writes drain; joining makes
        // the connection close before `stop` returns, so a reopen sees a released file.
        lock(&self.queue).take();
        let thread = lock(&self.thread).take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `None` once closed.
struct ReadPool {
    conns: Vec<Mutex<Option<Connection>>>,
    next: AtomicUsize,
}

/// A panic while holding the lock leaves the guarded value intact, so poisoning is ignored.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One writer thread plus a small read pool per database file (D6, architecture §5.2): writes
/// are serialized by construction, so `SQLITE_BUSY` never reaches callers.
#[derive(Clone)]
pub struct DbHandle {
    writer: Arc<Writer>,
    readers: Arc<ReadPool>,
}

impl DbHandle {
    /// `init` runs on the writer connection before any reader opens, so readers never observe
    /// a half-migrated schema.
    pub(crate) fn open(
        path: &Path,
        kind: DbKind,
        init: impl FnOnce(&mut Connection) -> Result<(), StoreError>,
    ) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        apply_pragmas(&conn, kind)?;
        init(&mut conn)?;

        let readers = (0..READ_POOL_SIZE)
            .map(|_| open_reader(path).map(|c| Mutex::new(Some(c))))
            .collect::<Result<Vec<_>, _>>()?;

        let (queue, jobs) = crossbeam_channel::bounded::<WriteJob>(WRITE_QUEUE_CAPACITY);
        let thread = std::thread::Builder::new()
            .name(format!("wolluf-db-writer-{kind:?}"))
            .spawn(move || writer_loop(conn, &jobs))
            .map_err(|e| StoreError::io(path, e))?;

        Ok(Self {
            writer: Arc::new(Writer {
                queue: Mutex::new(Some(queue)),
                thread: Mutex::new(Some(thread)),
            }),
            readers: Arc::new(ReadPool {
                conns: readers,
                next: AtomicUsize::new(0),
            }),
        })
    }

    /// Runs `f` in one transaction on the writer thread and blocks until it finishes. The app
    /// calls this from `spawn_blocking`; the store has no async runtime.
    pub fn write<R, F>(&self, f: F) -> Result<R, StoreError>
    where
        F: FnOnce(&Tx<'_>) -> Result<R, StoreError> + Send + 'static,
        R: Send + 'static,
    {
        let queue = lock(&self.writer.queue).clone().ok_or(StoreError::Closed)?;
        let (reply, result) = crossbeam_channel::bounded(1);
        let job: WriteJob = Box::new(move |conn| {
            let outcome = catch_unwind(AssertUnwindSafe(|| run_in_tx(conn, f)))
                .unwrap_or_else(|payload| Err(StoreError::WriterPanicked(panic_text(&*payload))));
            let _ = reply.send(outcome);
        });
        queue.send(job).map_err(|_| StoreError::WriterGone)?;
        // A held sender keeps the writer loop alive, which would stall `close` until the reply.
        drop(queue);
        result.recv().map_err(|_| StoreError::WriterGone)?
    }

    pub fn read<R>(
        &self,
        f: impl FnOnce(Conn<'_>) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        let pool = &self.readers;
        let start = pool.next.fetch_add(1, Ordering::Relaxed) % pool.conns.len();
        let free = (0..pool.conns.len())
            .map(|i| &pool.conns[(start + i) % pool.conns.len()])
            .find_map(|m| m.try_lock().ok());
        let guard = match free {
            Some(guard) => guard,
            None => lock(&pool.conns[start]),
        };
        let conn = guard.as_ref().ok_or(StoreError::Closed)?;
        f(Conn(conn))
    }

    /// Blocking. Drains and joins the writer, then closes the readers, for every clone; later
    /// calls get `Closed`. The order matches a plain drop: a read-only connection closing last
    /// never checkpoints, so the on-disk main file and WAL are the same either way.
    pub fn close(&self) {
        self.writer.stop();
        for conn in &self.readers.conns {
            lock(conn).take();
        }
    }
}

fn apply_pragmas(conn: &Connection, kind: DbKind) -> Result<(), StoreError> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    let mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::InvalidData(format!(
            "journal_mode stayed {mode:?} instead of wal"
        )));
    }
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "synchronous", kind.synchronous())?;
    Ok(())
}

fn open_reader(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(conn)
}

fn writer_loop(mut conn: Connection, jobs: &Receiver<WriteJob>) {
    for job in jobs {
        job(&mut conn);
    }
}

fn run_in_tx<R>(
    conn: &mut Connection,
    f: impl FnOnce(&Tx<'_>) -> Result<R, StoreError>,
) -> Result<R, StoreError> {
    // IMMEDIATE takes the write lock up front: a deferred transaction that reads first could
    // fail to upgrade if another process ever wrote in between.
    let tx = Tx(conn.transaction_with_behavior(TransactionBehavior::Immediate)?);
    let value = f(&tx)?;
    tx.0.commit()?;
    Ok(value)
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const THREADS: usize = 8;
    const INSERTS_PER_THREAD: usize = 1_000;

    pub(crate) fn counter_db(dir: &Path, kind: DbKind) -> DbHandle {
        DbHandle::open(&dir.join("t.db"), kind, |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS t(id INTEGER PRIMARY KEY, v INTEGER NOT NULL) STRICT;",
            )?;
            Ok(())
        })
        .unwrap()
    }

    fn insert(db: &DbHandle, v: i64) -> Result<(), StoreError> {
        db.write(move |tx| {
            tx.0.execute("INSERT INTO t(v) VALUES (?1)", [v])?;
            Ok(())
        })
    }

    fn count(db: &DbHandle) -> i64 {
        db.read(|c| Ok(c.0.query_row("SELECT count(*) FROM t", [], |r| r.get(0))?))
            .unwrap()
    }

    #[test]
    fn concurrent_writes_serialized() {
        let dir = tempfile::tempdir().unwrap();
        // NORMAL sync keeps 8,000 commits fast; this test is about serialization, not fsync.
        let db = counter_db(dir.path(), DbKind::Cache);
        let errors: usize = std::thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|t| {
                    let db = db.clone();
                    s.spawn(move || {
                        (0..INSERTS_PER_THREAD)
                            .filter(|i| insert(&db, (t * INSERTS_PER_THREAD + i) as i64).is_err())
                            .count()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum()
        });
        assert_eq!(errors, 0);
        assert_eq!(count(&db), (THREADS * INSERTS_PER_THREAD) as i64);
    }

    #[test]
    fn readers_see_committed_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = counter_db(dir.path(), DbKind::User);
        assert_eq!(count(&db), 0);
        insert(&db, 7).unwrap();
        // Every pooled reader must see the commit, not only the one that happened to be warm.
        for _ in 0..8 {
            assert_eq!(count(&db), 1);
        }
    }

    #[test]
    fn failed_write_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let db = counter_db(dir.path(), DbKind::User);
        let res: Result<(), StoreError> = db.write(|tx| {
            tx.0.execute("INSERT INTO t(v) VALUES (1)", [])?;
            Err(StoreError::InvalidData("abort".into()))
        });
        assert!(res.is_err());
        assert_eq!(count(&db), 0);
    }

    #[test]
    fn panicking_write_keeps_writer_alive() {
        let dir = tempfile::tempdir().unwrap();
        let db = counter_db(dir.path(), DbKind::User);
        let res: Result<(), StoreError> = db.write(|_| panic!("boom"));
        assert!(matches!(res, Err(StoreError::WriterPanicked(ref m)) if m.contains("boom")));
        insert(&db, 1).unwrap();
        assert_eq!(count(&db), 1);
    }

    #[test]
    fn close_ends_every_clone() {
        let dir = tempfile::tempdir().unwrap();
        let db = counter_db(dir.path(), DbKind::User);
        let other = db.clone();
        insert(&db, 1).unwrap();
        db.close();
        assert!(matches!(insert(&other, 2), Err(StoreError::Closed)));
        assert!(matches!(other.read(|_| Ok(())), Err(StoreError::Closed)));
        other.close();
    }

    fn pragma<T: rusqlite::types::FromSql + Send + 'static>(
        db: &DbHandle,
        name: &'static str,
    ) -> T {
        db.write(move |tx| {
            Ok(tx
                .0
                .query_row(&format!("PRAGMA {name}"), [], |r| r.get(0))?)
        })
        .unwrap()
    }

    #[test]
    fn pragmas_applied() {
        let dir = tempfile::tempdir().unwrap();
        for (kind, file, sync) in [(DbKind::User, "u.db", 2_i64), (DbKind::Cache, "c.db", 1)] {
            let db = DbHandle::open(&dir.path().join(file), kind, |_| Ok(())).unwrap();
            assert_eq!(pragma::<String>(&db, "journal_mode"), "wal");
            assert_eq!(pragma::<i64>(&db, "foreign_keys"), 1);
            assert_eq!(pragma::<i64>(&db, "synchronous"), sync, "{kind:?}");
            assert_eq!(pragma::<i64>(&db, "busy_timeout"), 5_000);
        }
    }
}
