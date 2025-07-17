//! libdplyr CLI binary
//!
//! A command-line tool for converting R dplyr syntax to SQL.

use libdplyr::cli::run_cli;
use std::process;

fn main() {
    let exit_code = run_cli();
    process::exit(exit_code);
}
