use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process;

use cadence_core::db::{self, DbOpen, Health};
use cadence_core::db_access::DbAccess;
use cadence_lib::api::lifecycle::ApiLifecycle;

enum Phase {
    Enable,
    Startup,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("api_fatal_harness: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let (phase, database_path, discovery_path) = parse_args()?;
    let database = match db::open_app(&database_path, Health::exit_process(1))
        .map_err(|error| error.to_string())?
    {
        DbOpen::Ready(database) => database,
        DbOpen::NeedsMigration(mut database, version) => {
            if version == 0 {
                db::schema::create_tables(&database.conn).map_err(|error| error.to_string())?;
            }
            db::migrate(&mut database.conn).map_err(|error| error.to_string())?;
            database
        }
        DbOpen::SchemaNewer {
            db_version,
            supported,
        } => {
            return Err(format!(
                "database schema v{db_version} is newer than supported v{supported}"
            ));
        }
        DbOpen::MissingFile => return Err("open_app unexpectedly reported a missing file".into()),
    };

    assert_database_ready(&database)?;
    let health = database.health.clone();
    let main = DbAccess::new(database);
    let mut lifecycle = ApiLifecycle::new(database_path, discovery_path, main);
    let status = match phase {
        Phase::Enable => lifecycle.set_enabled(true).await,
        Phase::Startup => lifecycle.startup().await,
    }
    .map_err(|error| format!("{error:?}"))?;
    let port = status.port.ok_or_else(|| "API did not start".to_string())?;

    {
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "READY {port}").map_err(|error| error.to_string())?;
        stdout.flush().map_err(|error| error.to_string())?;
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let mut command = String::new();
        let read_result = io::stdin().lock().read_line(&mut command);
        if read_result.is_ok() && command.trim_end() == "POISON" {
            health.poison("harness trigger");
        }
        let _ = shutdown_tx.send(());
    });

    let _ = shutdown_rx.await;
    lifecycle
        .shutdown_on_exit()
        .await
        .map_err(|error| format!("{error:?}"))
}

fn parse_args() -> Result<(Phase, PathBuf, PathBuf), String> {
    let mut args = env::args_os();
    let _program = args.next();
    let phase = match args.next().as_deref() {
        Some(value) if value == "enable" => Phase::Enable,
        Some(value) if value == "startup" => Phase::Startup,
        _ => {
            return Err(
                "usage: api_fatal_harness <enable|startup> <db_path> <discovery_path>".into(),
            )
        }
    };
    let database_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "missing db_path".to_string())?;
    let discovery_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "missing discovery_path".to_string())?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    Ok((phase, database_path, discovery_path))
}

fn assert_database_ready(database: &cadence_core::db::Db) -> Result<(), String> {
    let version: i64 = database
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if version != db::CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "database schema v{version}, expected v{}",
            db::CURRENT_SCHEMA_VERSION
        ));
    }

    let journal_mode: String = database
        .conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(format!(
            "database journal mode is {journal_mode}, expected wal"
        ));
    }
    Ok(())
}
