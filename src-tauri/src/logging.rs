use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Condvar, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

const LIMIT: u64 = 5 * 1024 * 1024;
const MAX_RECORDS: usize = 2000;
const MAX_BYTES: usize = 1024 * 1024;
const INTERVAL: Duration = Duration::from_secs(60);
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub level: String,
    pub message: String,
    pub timestamp: u64,
    pub cycle: bool,
}
struct Record {
    entry: Entry,
    line: Vec<u8>,
    cost: usize,
}
impl Record {
    fn new(entry: Entry) -> Self {
        let mut line = serde_json::to_vec(&entry).expect("serializing log entry");
        line.push(b'\n');
        // Account for owned strings and queue/record overhead, as well as encoded data.
        let cost = line.capacity()
            + entry.id.capacity()
            + entry.level.capacity()
            + entry.message.capacity()
            + std::mem::size_of::<Record>();
        Self { entry, line, cost }
    }
}
enum Control {
    Flush,
    Read(mpsc::Sender<Result<Vec<Entry>, String>>),
    Clear(mpsc::Sender<Result<(), String>>),
    Shutdown(mpsc::Sender<Result<(), String>>),
}
#[derive(Default)]
struct State {
    pending: VecDeque<Record>,
    controls: VecDeque<Control>,
    count: usize,
    bytes: usize,
    inflight_count: usize,
    inflight_bytes: usize,
    generation: u64,
    deadline: Option<Instant>,
    overflow_reported: bool,
    closed: bool,
}
type Shared = Arc<(Mutex<State>, Condvar)>;
type Notify = Arc<dyn Fn(&str, String, Option<Entry>) + Send + Sync>;
pub struct Logger {
    shared: Shared,
    notify: Notify,
    interval: Duration,
}
impl Logger {
    pub fn new(app: AppHandle, dir: PathBuf) -> Self {
        let notify: Notify = Arc::new(move |kind, message, entry| {
            if let Some(entry) = entry {
                let _ = app.emit(kind, entry);
            } else if kind == "logs-cleared" {
                let _ = app.emit(kind, ());
            } else {
                let _ = app.emit(kind, message);
            }
        });
        Self::with_worker(dir, INTERVAL, notify)
    }
    fn with_worker(dir: PathBuf, interval: Duration, notify: Notify) -> Self {
        let shared: Shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker_shared = shared.clone();
        let worker_notify = notify.clone();
        std::thread::spawn(move || worker(worker_shared, dir, interval, worker_notify));
        Self {
            shared,
            notify,
            interval,
        }
    }
    pub fn write(&self, level: &str, message: String, cycle: bool) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let entry = Entry {
            id: format!(
                "{}-{}-{}",
                now.as_nanos(),
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
            level: level.into(),
            message,
            timestamp: now.as_secs(),
            cycle,
        };
        let urgent = level == "ERROR";
        let record = Record::new(entry);
        let (lock, cv) = &*self.shared;
        let mut state = lock.lock().unwrap();
        if state.closed {
            return;
        }
        if cycle {
            (self.notify)("cycle-log", String::new(), Some(record.entry.clone()));
        }
        let mut lost = false;
        while state.count + state.inflight_count + 1 > MAX_RECORDS
            || state.bytes + state.inflight_bytes + record.cost > MAX_BYTES
        {
            let index = state
                .pending
                .iter()
                .position(|r| r.entry.message.starts_with("OCR "))
                .or_else(|| {
                    if state.pending.is_empty() {
                        None
                    } else {
                        Some(0)
                    }
                });
            if let Some(index) = index {
                let removed = state.pending.remove(index).unwrap();
                state.count -= 1;
                state.bytes -= removed.cost;
                lost = true;
            } else {
                lost = true;
                break;
            }
        }
        if state.count + state.inflight_count + 1 <= MAX_RECORDS
            && state.bytes + state.inflight_bytes + record.cost <= MAX_BYTES
        {
            state.bytes += record.cost;
            state.count += 1;
            state.pending.push_back(record);
            if state.deadline.is_none() {
                state.deadline = Some(Instant::now() + self.interval);
            }
            if urgent && !state.controls.iter().any(|c| matches!(c, Control::Flush)) {
                state.controls.push_back(Control::Flush);
            }
        }
        let warn = lost && !state.overflow_reported;
        if warn {
            state.overflow_reported = true;
        }
        cv.notify_one();
        drop(state);
        if warn {
            (self.notify)("log-error", "คิว log เต็ม บางรายการไม่ได้บันทึก".into(), None);
        }
    }
    pub fn flush(&self) {
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().unwrap();
        if !s.closed && !s.controls.iter().any(|c| matches!(c, Control::Flush)) {
            s.controls.push_back(Control::Flush);
            cv.notify_one();
        }
    }
    pub fn read(&self) -> Result<Vec<Entry>, String> {
        let (tx, rx) = mpsc::channel();
        self.control(Control::Read(tx))?;
        rx.recv().map_err(|e| e.to_string())?
    }
    pub fn clear(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().map_err(|e| e.to_string())?;
        if s.closed {
            return Err("logger stopped".into());
        }
        s.generation += 1;
        s.pending.clear();
        s.count = 0;
        s.bytes = 0;
        s.deadline = None;
        s.overflow_reported = false;
        s.controls
            .retain(|control| !matches!(control, Control::Flush));
        s.controls.push_front(Control::Clear(tx));
        cv.notify_one();
        drop(s);
        rx.recv().map_err(|e| e.to_string())?
    }
    pub fn shutdown(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.control(Control::Shutdown(tx))?;
        rx.recv().map_err(|e| e.to_string())?
    }
    fn control(&self, control: Control) -> Result<(), String> {
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().map_err(|e| e.to_string())?;
        if s.closed {
            return Err("logger stopped".into());
        }
        s.controls.push_back(control);
        cv.notify_one();
        Ok(())
    }
}
fn flush_pending(
    shared: &Shared,
    dir: &Path,
    interval: Duration,
    notify: &Notify,
) -> Result<(), String> {
    let (lock, _) = &**shared;
    let (batch, generation) = {
        let mut s = lock.lock().unwrap();
        let batch: Vec<Record> = s.pending.drain(..).collect();
        s.inflight_count = s.count;
        s.inflight_bytes = s.bytes;
        s.count = 0;
        s.bytes = 0;
        s.deadline = None;
        (batch, s.generation)
    };
    if batch.is_empty() {
        return Ok(());
    }
    let outcome = append_batch(dir, &batch, LIMIT);
    let (written, error) = match outcome {
        Ok(n) => (n, None),
        Err((n, e)) => (n, Some(e)),
    };
    let mut s = lock.lock().unwrap();
    s.inflight_count = 0;
    s.inflight_bytes = 0;
    if error.is_some() && s.generation == generation {
        for record in batch.into_iter().skip(written).rev() {
            s.count += 1;
            s.bytes += record.cost;
            s.pending.push_front(record);
        }
        s.deadline = Some(Instant::now() + interval);
    } else if error.is_none() {
        s.overflow_reported = false;
    }
    drop(s);
    if let Some(e) = error {
        (notify)("log-error", format!("บันทึก log ไม่สำเร็จ: {e}"), None);
        Err(e)
    } else {
        Ok(())
    }
}
fn worker(shared: Shared, dir: PathBuf, interval: Duration, notify: Notify) {
    loop {
        let (lock, cv) = &*shared;
        let mut s = lock.lock().unwrap();
        while s.controls.is_empty() && s.deadline.map_or(true, |d| d > Instant::now()) {
            s = if let Some(deadline) = s.deadline {
                cv.wait_timeout(s, deadline.saturating_duration_since(Instant::now()))
                    .unwrap()
                    .0
            } else {
                cv.wait(s).unwrap()
            };
        }
        let control = s.controls.pop_front();
        drop(s);
        match control {
            None | Some(Control::Flush) => {
                let _ = flush_pending(&shared, &dir, interval, &notify);
            }
            Some(Control::Read(reply)) => {
                let result = read_cycles(&dir).map(|mut history| {
                    let s = lock.lock().unwrap();
                    history.extend(
                        s.pending
                            .iter()
                            .filter(|r| r.entry.cycle)
                            .map(|r| r.entry.clone()),
                    );
                    if history.len() > 100 {
                        history.drain(..history.len() - 100);
                    }
                    history
                });
                let _ = reply.send(result);
            }
            Some(Control::Clear(reply)) => {
                let result = clear(&dir);
                if result.is_ok() {
                    let s = lock.lock().unwrap();
                    (notify)("logs-cleared", String::new(), None);
                    for record in &s.pending {
                        if record.entry.cycle {
                            (notify)("cycle-log", String::new(), Some(record.entry.clone()));
                        }
                    }
                }
                let _ = reply.send(result);
            }
            Some(Control::Shutdown(reply)) => {
                {
                    let mut s = lock.lock().unwrap();
                    s.closed = true;
                }
                let result = flush_pending(&shared, &dir, interval, &notify);
                let success = result.is_ok();
                if !success {
                    lock.lock().unwrap().closed = false;
                }
                let _ = reply.send(result);
                if success {
                    break;
                }
            }
        }
    }
}

fn path(dir: &Path, index: u8) -> PathBuf {
    dir.join(if index == 0 {
        "cookie.log".into()
    } else {
        format!("cookie.log.{index}")
    })
}
fn append_batch(dir: &Path, entries: &[Record], limit: u64) -> Result<usize, (usize, String)> {
    fs::create_dir_all(dir).map_err(|e| (0, e.to_string()))?;
    let mut written = 0;
    while written < entries.len() {
        let current = path(dir, 0);
        let size = match fs::metadata(&current) {
            Ok(m) => m.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err((written, e.to_string())),
        };
        if size > 0 && size.saturating_add(entries[written].line.len() as u64) > limit {
            remove_if_exists(&path(dir, 2)).map_err(|e| (written, e))?;
            let older = path(dir, 1);
            if older.exists() {
                fs::rename(older, path(dir, 2)).map_err(|e| (written, e.to_string()))?;
            }
            fs::rename(&current, path(dir, 1)).map_err(|e| (written, e.to_string()))?;
            continue;
        }
        let mut end = written;
        let mut total = size;
        while end < entries.len()
            && (total == 0 || total.saturating_add(entries[end].line.len() as u64) <= limit)
        {
            total += entries[end].line.len() as u64;
            end += 1;
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(current)
            .map_err(|e| (written, e.to_string()))?;
        let mut chunk = Vec::with_capacity((total - size) as usize);
        for record in &entries[written..end] {
            chunk.extend_from_slice(&record.line);
        }
        if let Err(error) = f.write_all(&chunk) {
            if let Err(rollback) = f.set_len(size) {
                return Err((
                    end,
                    format!("{error}; rollback failed: {rollback}; some log entries may be lost"),
                ));
            }
            return Err((written, error.to_string()));
        }
        written = end;
    }
    Ok(written)
}
#[cfg(test)]
fn append(dir: &Path, entry: &Entry, limit: u64) -> Result<(), String> {
    append_batch(dir, &[Record::new(entry.clone())], limit)
        .map(|_| ())
        .map_err(|(_, e)| e)
}
fn remove_if_exists(p: &Path) -> Result<(), String> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
fn clear(dir: &Path) -> Result<(), String> {
    for i in 0..=2 {
        remove_if_exists(&path(dir, i))?;
    }
    Ok(())
}
fn read_cycles(dir: &Path) -> Result<Vec<Entry>, String> {
    if dir.exists() && !dir.is_dir() {
        return Err("log path is not a directory".into());
    }
    let mut entries = VecDeque::new();
    for i in (0..=2).rev() {
        let file = match fs::File::open(path(dir, i)) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|e| e.to_string())?;
            if let Ok(entry) = serde_json::from_str::<Entry>(&line) {
                if entry.cycle {
                    entries.push_back(entry);
                    if entries.len() > 100 {
                        entries.pop_front();
                    }
                }
            }
        }
    }
    Ok(entries.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "cookie-log-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn entry(n: usize, cycle: bool) -> Entry {
        Entry {
            id: n.to_string(),
            level: "INFO".into(),
            message: format!("entry {n}"),
            timestamp: n as u64,
            cycle,
        }
    }
    #[test]
    fn rotation_keeps_two_old_files_and_clear_removes_all() {
        let dir = temp();
        for n in 0..4 {
            append(&dir, &entry(n, true), 1).unwrap_or_else(|e| panic!("entry {n}: {e}"));
        }
        let result = read_cycles(&dir).unwrap();
        assert_eq!(
            result.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
        clear(&dir).unwrap();
        assert!(read_cycles(&dir).unwrap().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn history_includes_only_latest_hundred_cycles() {
        let dir = temp();
        for n in 0..105 {
            append(&dir, &entry(n, true), LIMIT).unwrap();
        }
        append(&dir, &entry(999, false), LIMIT).unwrap();
        let result = read_cycles(&dir).unwrap();
        assert_eq!(result.len(), 100);
        assert_eq!(result[0].id, "5");
        assert_eq!(result[99].id, "104");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reports_filesystem_errors() {
        let dir = temp();
        let file = dir.join("not-a-directory");
        fs::write(&file, "x").unwrap();
        assert!(append(&file, &entry(1, true), LIMIT).is_err());
        assert!(read_cycles(&file).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    fn notifier() -> (Notify, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel();
        (
            Arc::new(move |kind, _, _| {
                let _ = tx.send(kind.into());
            }),
            rx,
        )
    }
    #[test]
    fn timer_flushes_and_cycle_is_emitted_immediately() {
        let dir = temp();
        let (notify, events) = notifier();
        let logger = Logger::with_worker(dir.clone(), Duration::from_millis(100), notify);
        logger.write("INFO", "cycle".into(), true);
        assert_eq!(
            events.recv_timeout(Duration::from_millis(50)).unwrap(),
            "cycle-log"
        );
        assert!(!path(&dir, 0).exists());
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path(&dir, 0).exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(read_cycles(&dir).unwrap().len(), 1);
        logger.shutdown().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn clear_discards_buffer_and_shutdown_flushes_new_entries() {
        let dir = temp();
        let (notify, _) = notifier();
        let logger = Logger::with_worker(dir.clone(), Duration::from_secs(60), notify);
        logger.write("INFO", "old".into(), true);
        assert_eq!(logger.read().unwrap().len(), 1);
        logger.clear().unwrap();
        logger.write("INFO", "new".into(), true);
        logger.shutdown().unwrap();
        let history = read_cycles(&dir).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "new");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn urgent_error_and_stop_flush_pending() {
        let dir = temp();
        let (notify, _) = notifier();
        let logger = Logger::with_worker(dir.clone(), Duration::from_secs(60), notify);
        logger.write("INFO", "first".into(), true);
        logger.write("ERROR", "problem".into(), false);
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path(&dir, 0).exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(read_cycles(&dir).unwrap().len(), 1);
        logger.write("INFO", "second".into(), true);
        logger.flush();
        let deadline = Instant::now() + Duration::from_secs(3);
        while read_cycles(&dir).unwrap().len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(read_cycles(&dir).unwrap().len(), 2);
        logger.shutdown().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bounded_queue_drops_ocr_first_even_if_writer_stalled() {
        let (notify, events) = notifier();
        let shared: Shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let logger = Logger {
            shared: shared.clone(),
            notify,
            interval: Duration::from_secs(60),
        };
        logger.write("INFO", "important".into(), false);
        for n in 0..MAX_RECORDS + 10 {
            logger.write("INFO", format!("OCR {n}"), false);
        }
        let state = shared.0.lock().unwrap();
        assert!(state.count <= MAX_RECORDS && state.bytes <= MAX_BYTES);
        assert_eq!(state.pending.front().unwrap().entry.message, "important");
        assert_eq!(
            events.recv_timeout(Duration::from_millis(50)).unwrap(),
            "log-error"
        );
        assert!(events.try_recv().is_err());
    }
    #[test]
    fn write_failure_retains_bounded_entries_and_retries() {
        let dir = temp();
        let blocked = dir.join("blocked");
        fs::write(&blocked, "x").unwrap();
        let (notify, events) = notifier();
        let logger = Logger::with_worker(blocked.clone(), Duration::from_millis(80), notify);
        logger.write("INFO", "will retry".into(), true);
        assert_eq!(
            events.recv_timeout(Duration::from_secs(1)).unwrap(),
            "cycle-log"
        );
        assert_eq!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            "log-error"
        );
        {
            let s = logger.shared.0.lock().unwrap();
            assert_eq!(s.count, 1);
            assert!(s.bytes <= MAX_BYTES);
        }
        fs::remove_file(&blocked).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path(&blocked, 0).exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(read_cycles(&blocked).unwrap().len(), 1);
        logger.shutdown().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn in_flight_data_reserves_queue_budget_without_blocking_producer() {
        let (notify, events) = notifier();
        let shared: Shared = Arc::new((
            Mutex::new(State {
                inflight_bytes: MAX_BYTES - 10,
                inflight_count: 1,
                ..State::default()
            }),
            Condvar::new(),
        ));
        let logger = Logger {
            shared: shared.clone(),
            notify,
            interval: Duration::from_secs(60),
        };
        logger.write("INFO", "OCR while disk stalled".into(), false);
        let state = shared.0.lock().unwrap();
        assert_eq!(state.count, 0);
        assert!(state.bytes + state.inflight_bytes <= MAX_BYTES);
        assert_eq!(
            events.recv_timeout(Duration::from_millis(50)).unwrap(),
            "log-error"
        );
    }
    #[test]
    fn batch_rotation_retains_complete_ordered_records() {
        let dir = temp();
        let records: Vec<_> = (0..4).map(|n| Record::new(entry(n, true))).collect();
        let limit = (records[0].line.len() + records[1].line.len()) as u64;
        append_batch(&dir, &records, limit).unwrap();
        let history = read_cycles(&dir).unwrap();
        assert_eq!(
            history.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["0", "1", "2", "3"]
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_shutdown_keeps_buffer_for_retry() {
        let dir = temp();
        let blocked = dir.join("blocked");
        fs::write(&blocked, "x").unwrap();
        let (notify, _) = notifier();
        let logger = Logger::with_worker(blocked.clone(), Duration::from_secs(60), notify);
        logger.write("INFO", "keep me".into(), true);
        assert!(logger.shutdown().is_err());
        fs::remove_file(&blocked).unwrap();
        logger.shutdown().unwrap();
        assert_eq!(read_cycles(&blocked).unwrap()[0].message, "keep me");
        fs::remove_dir_all(dir).unwrap();
    }
}
