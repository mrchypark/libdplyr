//! Parser module.
//!
//! Exposes the public AST and parsing API while keeping implementation split across
//! smaller modules.

pub mod ast;
pub mod parse;

pub use ast::*;
pub use parse::Parser;
