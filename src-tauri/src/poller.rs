use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use cadence_core::db::{open_peer, Db, DbOpen, Health};
use tauri::{Emitter as _, Manager as _};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_CONSECUTIVE_READ_FAILURES: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emit {
    DbChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollerState {
    last_seen: i64,
    consecutive_read_failures: u8,
}

impl PollerState {
    pub fn new(baseline: i64) -> Self {
        Self {
            last_seen: baseline,
            consecutive_read_failures: 0,
        }
    }

    pub fn tick(&mut self, visible: bool, data_version: Option<i64>) -> Option<Emit> {
        if !visible {
            return None;
        }

        let Some(data_version) = data_version else {
            self.consecutive_read_failures = self.consecutive_read_failures.saturating_add(1);
            return None;
        };

        self.consecutive_read_failures = 0;
        if data_version == self.last_seen {
            return None;
        }

        self.last_seen = data_version;
        Some(Emit::DbChanged)
    }

    fn read_failures_exhausted(&self) -> bool {
        self.consecutive_read_failures >= MAX_CONSECUTIVE_READ_FAILURES
    }
}

pub trait Visibility {
    fn any_visible(&self) -> bool;
}

pub trait Emitter {
    fn emit(&self, event: Emit) -> Result<(), String>;
}

pub trait Clock {
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}

pub trait DataVersionReader {
    fn read(&self) -> Result<i64, String>;
}

pub trait WarningSink {
    fn warn(&self, message: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollerOutcome {
    ClockStopped,
    ReadFailures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollerReport {
    pub outcome: PollerOutcome,
}

pub struct PollerDriver<V, E, C, R, W> {
    visibility: V,
    emitter: E,
    clock: C,
    reader: R,
    warnings: W,
    state: PollerState,
}

impl<V, E, C, R, W> PollerDriver<V, E, C, R, W>
where
    V: Visibility,
    E: Emitter,
    C: Clock,
    R: DataVersionReader,
    W: WarningSink,
{
    pub fn new(visibility: V, emitter: E, clock: C, reader: R, warnings: W, baseline: i64) -> Self {
        Self {
            visibility,
            emitter,
            clock,
            reader,
            warnings,
            state: PollerState::new(baseline),
        }
    }

    pub async fn run(mut self) -> PollerReport {
        while self.clock.wait().await {
            if !self.visibility.any_visible() {
                let _ = self.state.tick(false, None);
                continue;
            }

            let data_version = match self.reader.read() {
                Ok(version) => version,
                Err(error) => {
                    eprintln!("Database change poll error: {error}");
                    let _ = self.state.tick(true, None);
                    if self.state.read_failures_exhausted() {
                        self.warnings.warn(
                            "database change poller stopped after three consecutive read failures",
                        );
                        return PollerReport {
                            outcome: PollerOutcome::ReadFailures,
                        };
                    }
                    continue;
                }
            };

            let previous = self.state.clone();
            if let Some(event) = self.state.tick(true, Some(data_version)) {
                if let Err(error) = self.emitter.emit(event) {
                    self.state = previous;
                    eprintln!("Database change emit error: {error}");
                }
            }
        }

        PollerReport {
            outcome: PollerOutcome::ClockStopped,
        }
    }
}

pub struct ProductionDataVersionReader {
    db: Db,
}

impl ProductionDataVersionReader {
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = match open_peer(path, Health::exit_process(1))
            .map_err(|error| format!("could not open poller database: {error}"))?
        {
            DbOpen::Ready(db) => db,
            DbOpen::MissingFile => {
                return Err("poller database file is missing".to_string());
            }
            DbOpen::NeedsMigration(_, version) => {
                return Err(format!("poller database schema v{version} needs migration"));
            }
            DbOpen::SchemaNewer {
                db_version,
                supported,
            } => {
                return Err(format!(
                    "poller database schema v{db_version} is newer than supported v{supported}"
                ));
            }
        };

        db.conn
            .pragma_update(None, "query_only", true)
            .map_err(|error| format!("could not enable poller query-only mode: {error}"))?;
        if !db
            .conn
            .pragma_query_value(None, "query_only", |row| row.get::<_, bool>(0))
            .map_err(|error| format!("could not verify poller query-only mode: {error}"))?
        {
            return Err("poller query-only mode was not enabled".to_string());
        }

        Ok(Self { db })
    }

    #[cfg(test)]
    fn query_only(&self) -> Result<bool, String> {
        self.db
            .conn
            .pragma_query_value(None, "query_only", |row| row.get(0))
            .map_err(|error| error.to_string())
    }
}

impl DataVersionReader for ProductionDataVersionReader {
    fn read(&self) -> Result<i64, String> {
        self.db
            .conn
            .pragma_query_value(None, "data_version", |row| row.get(0))
            .map_err(|error| error.to_string())
    }
}

struct TauriVisibility {
    app: tauri::AppHandle,
}

impl TauriVisibility {
    fn window_visible(&self, label: &str) -> bool {
        let Some(window) = self.app.get_webview_window(label) else {
            return false;
        };
        match window.is_visible() {
            Ok(visible) => visible,
            Err(error) => {
                eprintln!("Database change visibility error for {label}: {error}");
                false
            }
        }
    }
}

impl Visibility for TauriVisibility {
    fn any_visible(&self) -> bool {
        let main_visible = self.window_visible("main");
        let search_visible = self.window_visible("search");
        main_visible || search_visible
    }
}

struct TauriEmitter {
    app: tauri::AppHandle,
}

impl Emitter for TauriEmitter {
    fn emit(&self, event: Emit) -> Result<(), String> {
        match event {
            Emit::DbChanged => self
                .app
                .emit("db-changed", ())
                .map_err(|error| error.to_string()),
        }
    }
}

struct TokioClock;

impl Clock for TokioClock {
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async {
            tokio::time::sleep(POLL_INTERVAL).await;
            true
        })
    }
}

struct StderrWarningSink;

impl WarningSink for StderrWarningSink {
    fn warn(&self, message: &str) {
        eprintln!("Warning: {message}");
    }
}

pub fn start(app: tauri::AppHandle, database_path: &Path) -> Result<(), String> {
    let reader = ProductionDataVersionReader::open(database_path)?;
    let baseline = reader.read()?;
    let driver = PollerDriver::new(
        TauriVisibility { app: app.clone() },
        TauriEmitter { app },
        TokioClock,
        reader,
        StderrWarningSink,
        baseline,
    );
    tauri::async_runtime::spawn(async move {
        let _ = driver.run().await;
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use cadence_core::db::{migrate, open_app, schema, DbOpen, Health};

    use super::*;

    struct TempDatabase {
        dir: PathBuf,
        path: PathBuf,
        writer: Db,
    }

    impl TempDatabase {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let dir = std::env::temp_dir().join(format!(
                "cadence-poller-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).expect("create poller temp directory");
            let path = dir.join("cadence.db");
            let mut writer = match open_app(&path, Health::exit_process(1))
                .expect("open poller fixture database")
            {
                DbOpen::NeedsMigration(db, 0) => db,
                _ => panic!("new poller fixture should need migration from v0"),
            };
            schema::create_tables(&writer.conn).expect("create poller fixture schema");
            migrate(&mut writer.conn).expect("migrate poller fixture");
            writer
                .conn
                .execute_batch("CREATE TABLE poller_probe (value INTEGER NOT NULL);")
                .expect("create poller probe");
            Self { dir, path, writer }
        }

        fn commit(&self, value: i64) {
            self.writer
                .conn
                .execute("INSERT INTO poller_probe (value) VALUES (?1)", [value])
                .expect("commit external poller change");
        }
    }

    impl Drop for TempDatabase {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[derive(Clone)]
    struct FixedVisibility(Arc<AtomicBool>);

    impl FixedVisibility {
        fn new(visible: bool) -> Self {
            Self(Arc::new(AtomicBool::new(visible)))
        }
    }

    impl Visibility for FixedVisibility {
        fn any_visible(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    struct ScriptedVisibility(Mutex<VecDeque<bool>>);

    impl Visibility for ScriptedVisibility {
        fn any_visible(&self) -> bool {
            self.0
                .lock()
                .expect("visibility script lock")
                .pop_front()
                .unwrap_or(false)
        }
    }

    #[derive(Clone, Default)]
    struct RecordingEmitter(Arc<AtomicUsize>);

    impl RecordingEmitter {
        fn count(&self) -> usize {
            self.0.load(Ordering::SeqCst)
        }
    }

    #[derive(Clone, Default)]
    struct RecordingWarningSink(Arc<AtomicUsize>);

    impl RecordingWarningSink {
        fn count(&self) -> usize {
            self.0.load(Ordering::SeqCst)
        }
    }

    impl WarningSink for RecordingWarningSink {
        fn warn(&self, message: &str) {
            assert_eq!(
                message,
                "database change poller stopped after three consecutive read failures"
            );
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Emitter for RecordingEmitter {
        fn emit(&self, event: Emit) -> Result<(), String> {
            assert_eq!(event, Emit::DbChanged);
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FiniteClock {
        remaining: usize,
        ticks: Arc<AtomicUsize>,
    }

    impl FiniteClock {
        fn new(ticks: usize) -> (Self, Arc<AtomicUsize>) {
            let observed = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    remaining: ticks,
                    ticks: observed.clone(),
                },
                observed,
            )
        }
    }

    impl Clock for FiniteClock {
        fn wait(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(async move {
                if self.remaining == 0 {
                    return false;
                }
                self.remaining -= 1;
                self.ticks.fetch_add(1, Ordering::SeqCst);
                true
            })
        }
    }

    struct CountingScriptedReader {
        inner: ProductionDataVersionReader,
        reads: Arc<AtomicUsize>,
        failures: Mutex<VecDeque<bool>>,
    }

    impl CountingScriptedReader {
        fn new(
            inner: ProductionDataVersionReader,
            failures: impl IntoIterator<Item = bool>,
        ) -> (Self, Arc<AtomicUsize>) {
            let reads = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    inner,
                    reads: reads.clone(),
                    failures: Mutex::new(failures.into_iter().collect()),
                },
                reads,
            )
        }
    }

    impl DataVersionReader for CountingScriptedReader {
        fn read(&self) -> Result<i64, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let should_fail = self
                .failures
                .lock()
                .map_err(|_| "script lock poisoned".to_string())?
                .pop_front()
                .unwrap_or(false);
            if should_fail {
                return Err("scripted read failure".to_string());
            }
            self.inner.read()
        }
    }

    type TestPollerDriver = PollerDriver<
        FixedVisibility,
        RecordingEmitter,
        FiniteClock,
        CountingScriptedReader,
        RecordingWarningSink,
    >;

    struct DriverFixture {
        driver: TestPollerDriver,
        reads: Arc<AtomicUsize>,
        clock_ticks: Arc<AtomicUsize>,
        warnings: RecordingWarningSink,
    }

    fn driver(
        path: &Path,
        visible: FixedVisibility,
        emitter: RecordingEmitter,
        ticks: usize,
        failures: impl IntoIterator<Item = bool>,
    ) -> DriverFixture {
        let production =
            ProductionDataVersionReader::open(path).expect("open production poller reader");
        let baseline = production.read().expect("read poller baseline");
        let (reader, reads) = CountingScriptedReader::new(production, failures);
        let (clock, clock_ticks) = FiniteClock::new(ticks);
        let warnings = RecordingWarningSink::default();
        DriverFixture {
            driver: PollerDriver::new(visible, emitter, clock, reader, warnings.clone(), baseline),
            reads,
            clock_ticks,
            warnings,
        }
    }

    #[test]
    fn reducer_preserves_baseline_while_hidden_and_emits_on_change() {
        let mut state = PollerState::new(7);
        assert_eq!(state.tick(false, Some(8)), None);
        assert_eq!(state.tick(true, Some(8)), Some(Emit::DbChanged));
        assert_eq!(state.tick(true, Some(8)), None);
    }

    #[test]
    fn reducer_tracks_only_visible_read_failures() {
        let mut state = PollerState::new(7);
        assert_eq!(state.tick(false, None), None);
        assert_eq!(state.consecutive_read_failures, 0);
        assert_eq!(state.tick(true, None), None);
        assert_eq!(state.consecutive_read_failures, 1);
        assert_eq!(state.tick(true, Some(7)), None);
        assert_eq!(state.consecutive_read_failures, 0);
    }

    #[test]
    fn dedicated_reader_enables_and_verifies_query_only() {
        let fixture = TempDatabase::new();
        let reader =
            ProductionDataVersionReader::open(&fixture.path).expect("open production reader");
        assert!(reader.query_only().expect("read query-only pragma"));
    }

    #[tokio::test]
    async fn external_commit_while_visible_emits_within_one_tick() {
        let fixture = TempDatabase::new();
        let visible = FixedVisibility::new(true);
        let emitter = RecordingEmitter::default();
        let fixture_driver = driver(&fixture.path, visible, emitter.clone(), 1, []);
        fixture.commit(1);

        let report = fixture_driver.driver.run().await;

        assert_eq!(report.outcome, PollerOutcome::ClockStopped);
        assert_eq!(fixture_driver.clock_ticks.load(Ordering::SeqCst), 1);
        assert_eq!(fixture_driver.reads.load(Ordering::SeqCst), 1);
        assert_eq!(emitter.count(), 1);
    }

    #[tokio::test]
    async fn hidden_ticks_perform_no_database_reads() {
        let fixture = TempDatabase::new();
        let visible = FixedVisibility::new(false);
        let emitter = RecordingEmitter::default();
        let fixture_driver = driver(&fixture.path, visible, emitter.clone(), 3, []);

        let report = fixture_driver.driver.run().await;

        assert_eq!(report.outcome, PollerOutcome::ClockStopped);
        assert_eq!(fixture_driver.reads.load(Ordering::SeqCst), 0);
        assert_eq!(emitter.count(), 0);
    }

    #[tokio::test]
    async fn hidden_commit_emits_on_first_visible_tick() {
        let fixture = TempDatabase::new();
        let emitter = RecordingEmitter::default();
        let production =
            ProductionDataVersionReader::open(&fixture.path).expect("open production reader");
        let baseline = production.read().expect("read poller baseline");
        let (reader, reads) = CountingScriptedReader::new(production, []);
        let (clock, _) = FiniteClock::new(2);
        fixture.commit(1);
        let driver = PollerDriver::new(
            ScriptedVisibility(Mutex::new(VecDeque::from([false, true]))),
            emitter.clone(),
            clock,
            reader,
            RecordingWarningSink::default(),
            baseline,
        );

        driver.run().await;

        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(emitter.count(), 1);
    }

    #[tokio::test]
    async fn unchanged_reads_do_not_emit() {
        let fixture = TempDatabase::new();
        let visible = FixedVisibility::new(true);
        let emitter = RecordingEmitter::default();
        let fixture_driver = driver(&fixture.path, visible, emitter.clone(), 2, []);

        fixture_driver.driver.run().await;

        assert_eq!(fixture_driver.reads.load(Ordering::SeqCst), 2);
        assert_eq!(emitter.count(), 0);
    }

    #[tokio::test]
    async fn scripted_read_failure_is_treated_as_hidden_then_recovers() {
        let fixture = TempDatabase::new();
        let visible = FixedVisibility::new(true);
        let emitter = RecordingEmitter::default();
        let fixture_driver = driver(&fixture.path, visible, emitter.clone(), 2, [true, false]);
        fixture.commit(1);

        let report = fixture_driver.driver.run().await;

        assert_eq!(report.outcome, PollerOutcome::ClockStopped);
        assert_eq!(fixture_driver.reads.load(Ordering::SeqCst), 2);
        assert_eq!(emitter.count(), 1);
    }

    #[tokio::test]
    async fn three_consecutive_read_failures_stop_with_one_warning() {
        let fixture = TempDatabase::new();
        let visible = FixedVisibility::new(true);
        let emitter = RecordingEmitter::default();
        let fixture_driver = driver(
            &fixture.path,
            visible,
            emitter.clone(),
            5,
            [true, true, true],
        );

        let report = fixture_driver.driver.run().await;

        assert_eq!(report.outcome, PollerOutcome::ReadFailures);
        assert_eq!(fixture_driver.warnings.count(), 1);
        assert_eq!(fixture_driver.reads.load(Ordering::SeqCst), 3);
        assert_eq!(emitter.count(), 0);
    }
}
