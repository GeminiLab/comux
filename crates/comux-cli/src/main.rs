//! Binary entry point for the comux CLI and terminal dashboard.

use clap::Parser;
use comux::args::CliArgs;
use comux::commands::run;

/// Parses arguments, creates the async runtime, and dispatches the CLI.
///
/// Runtime or command failures are written to stderr and terminate the process
/// with a non-zero status.
fn main() {
    let args = CliArgs::parse();
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("comux: failed to initialize runtime: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = runtime.block_on(run(args)) {
        eprintln!("comux: {error}");
        std::process::exit(1);
    }
}
