//! libdplyr CLI binary
//!
//! A command-line tool for converting R dplyr syntax to SQL.

use libdplyr::cli::{run_cli, print_error};
use libdplyr::error::TranspileError;
use std::process;

fn main() {
    if let Err(error) = run_cli() {
        // Handle errors appropriately based on type
        if let Some(transpile_error) = error.downcast_ref::<TranspileError>() {
            print_error(transpile_error);
            process::exit(1);
        } else {
            eprintln!("Error: {}", error);
            process::exit(1);
        }
    }
}