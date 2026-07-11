//! Servicios de aplicación del launcher.
//!
//! Regla: toda feature con lógica de negocio vive aquí; `commands/` solo expone IPC Tauri.
//!
//! Capas:
//! - `ro-tools-core` / `ro-tools-linux` — dominio puro + adaptadores OS (game tools)
//! - `tools/*` — orquestación por feature
//! - `utils/*` — infra compartida (Wine, paths, eventos, JSON)
//! - `commands/*` — handlers delgados
//!
//! Añadir una feature nueva:
//! 1. Dominio reutilizable → `crates/ro-tools-*` (si aplica)
//! 2. Orquestación → `tools/<feature>/session.rs` (+ módulos internos)
//! 3. Handler → `commands/<feature>.rs`

pub mod autobuff;
pub mod autopot;
pub mod deps;
pub mod input;
pub mod launcher;
pub mod prefix;
pub mod runners;
pub mod server_tools;
pub mod spammer;
