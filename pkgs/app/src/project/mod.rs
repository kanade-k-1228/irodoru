//! Project lifetime, portable ZIP archives and single-writer autosave.
mod active;
mod archive;
mod charas;
mod export;
pub use export::ExportNaming;
pub(crate) use export::export_lines;
mod autosave;
pub mod incoming;
mod ownership;
mod session;
pub use active::Project;
use autosave::Saver;
pub use session::{ProjectStatus, SaveState, Session};
use session::{SharedProject, status};

/// Project paths share one rule across native entry points and storage.
pub fn is_project_path(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("iro"))
}
pub(crate) fn validate_path(path: &std::path::Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        is_project_path(path),
        "対応するプロジェクト形式は .iro のみです"
    );
    Ok(())
}
