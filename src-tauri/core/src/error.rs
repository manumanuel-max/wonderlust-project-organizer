use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Error de E/S en «{path}»: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("La ruta «{0}» ya existe")]
    AlreadyExists(PathBuf),
    #[error("La ruta «{0}» no existe")]
    NotFound(PathBuf),
    #[error("Datos inválidos: {0}")]
    Invalid(String),
    #[error("Operación cancelada")]
    Cancelled,
    #[error("Error de serialización: {0}")]
    Serde(#[from] serde_json::Error),
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        CoreError::Io {
            path: path.into(),
            source,
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
