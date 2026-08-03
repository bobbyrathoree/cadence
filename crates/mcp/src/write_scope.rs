use cadence_core::db::Db;

use crate::fault::{self, Fault};

pub struct WriteScope<'a> {
    db: &'a mut Db,
    closed: bool,
}

impl<'a> WriteScope<'a> {
    pub fn open(db: &'a mut Db) -> rusqlite::Result<Self> {
        db.conn.pragma_update(None, "query_only", false)?;
        Ok(Self { db, closed: false })
    }

    pub fn db(&mut self) -> &mut Db {
        self.db
    }

    pub fn close(mut self) -> rusqlite::Result<()> {
        self.closed = true;
        let result = if fault::active() == Some(Fault::FailWriteScopeClose) {
            Err(rusqlite::Error::InvalidQuery)
        } else {
            self.db.conn.pragma_update(None, "query_only", true)
        };
        if result.is_err() {
            self.db.health.poison("write scope close failed");
        }
        result
    }
}

impl Drop for WriteScope<'_> {
    fn drop(&mut self) {
        if !self.closed && std::thread::panicking() {
            let result = self.db.conn.pragma_update(None, "query_only", true);
            if result.is_err() {
                self.db.health.poison("write scope close failed");
            }
        }
    }
}
