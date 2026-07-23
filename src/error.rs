use std::path::PathBuf;

pub type SentraResult<T> = Result<T, SentraError>;

#[derive(Debug, thiserror::Error)]
pub enum SentraError {
    #[error("I/O error at {path:?}: {source}")]
    Io {
        path: Option<PathBuf>,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("SQLite error at {path:?}: {source}")]
    Sqlite {
        path: Option<PathBuf>,
        #[source]
        source: rusqlite::Error,
    },
    #[error("unsupported wire protocol: {0}")]
    UnsupportedProtocol(String),
    #[error("{0}")]
    Message(String),
}

impl SentraError {
    pub fn io(path: impl Into<Option<PathBuf>>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn sqlite(path: impl Into<Option<PathBuf>>, source: rusqlite::Error) -> Self {
        Self::Sqlite {
            path: path.into(),
            source,
        }
    }
}

impl From<std::io::Error> for SentraError {
    fn from(source: std::io::Error) -> Self {
        Self::io(None, source)
    }
}
