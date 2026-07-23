use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{OpenFlags, Params, Row};

use crate::{SentraError, SentraResult};

pub(crate) struct SqliteDatabase {
    connection: rusqlite::Connection,
    path: PathBuf,
}

impl SqliteDatabase {
    pub(crate) fn open_read_only(path: impl AsRef<Path>) -> SentraResult<Option<Self>> {
        let path = path.as_ref();
        if !path.is_file() {
            return Ok(None);
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = rusqlite::Connection::open_with_flags(path, flags)
            .map_err(|err| SentraError::sqlite(Some(path.to_path_buf()), err))?;
        connection
            .busy_timeout(Duration::from_millis(250))
            .map_err(|err| SentraError::sqlite(Some(path.to_path_buf()), err))?;
        Ok(Some(Self {
            connection,
            path: path.to_path_buf(),
        }))
    }

    pub(crate) fn table_exists(&self, table: &str) -> SentraResult<bool> {
        self.query_optional(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            [table],
            |_| Ok(()),
        )
        .map(|value| value.is_some())
    }

    pub(crate) fn query_map<T, P, F>(
        &self,
        sql: &str,
        params: P,
        mut mapper: F,
    ) -> SentraResult<Vec<T>>
    where
        P: Params,
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|err| SentraError::sqlite(Some(self.path.clone()), err))?;
        let rows = statement
            .query_map(params, |row| mapper(row))
            .map_err(|err| SentraError::sqlite(Some(self.path.clone()), err))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| SentraError::sqlite(Some(self.path.clone()), err))
    }

    pub(crate) fn query_optional<T, P, F>(
        &self,
        sql: &str,
        params: P,
        mapper: F,
    ) -> SentraResult<Option<T>>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    {
        match self.connection.query_row(sql, params, mapper) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(SentraError::sqlite(Some(self.path.clone()), err)),
        }
    }
}
