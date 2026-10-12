mod repository;
mod scan;
mod session;

pub use repository::InstallationManager;
pub use scan::scan_candidates;
pub use session::{confirm, start};

#[cfg(test)]
mod tests;
