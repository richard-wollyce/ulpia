//! kb, a linter, indexer and router for file based knowledge bases.
//!
//! ADR-0003 decided that the markdown files stay the source of truth and that any
//! index is derived from them. This crate is that derived thing: it stores nothing
//! it cannot recompute, reads the files on every run, and either reports what is
//! broken (`checks`) or answers what a question should open (`memory`).
//!
//! **Start at [`memory::Memory`].** It is the contract: three verbs over a set of
//! bases, and the one place an answer is computed. The `serve` subcommand wraps it
//! in MCP for other people's runtimes, the GUI links it and calls it directly, and a
//! hosted service later would wrap it in HTTP. Reaching past it into the modules
//! below means rebuilding the pipeline, which is how the two scorers came to see
//! different terms once already.

pub mod base;
pub mod blocks;
pub mod boot;
pub mod checks;
pub mod commit;
pub mod eval;
pub mod fleet;
pub mod index;
pub mod init;
pub mod json;
pub mod mcp;
pub mod memory;
pub mod misses;
pub mod remember;
pub mod retrieve;
pub mod store;
pub mod write;
