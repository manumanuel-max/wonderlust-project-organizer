//! Núcleo de Wonderlust Project Organizer.
//!
//! Este crate no depende de Tauri: contiene toda la lógica de clasificación,
//! planificación, verificación y copia segura, y se prueba con `cargo test`.
//!
//! Regla de oro: **ningún código de este crate escribe, mueve, renombra ni
//! elimina rutas de origen**. Los orígenes solo se abren en modo lectura.

pub mod category;
pub mod classify;
pub mod copy;
pub mod duplicates;
pub mod error;
pub mod exclusions;
pub mod folders;
pub mod hashing;
pub mod history;
pub mod naming;
pub mod paths;
pub mod plan;
pub mod preflight;
pub mod report;
pub mod scan;
pub mod sequence;
pub mod settings;
pub mod structure;

pub use category::Category;
pub use error::{CoreError, CoreResult};
