use clap::Parser;
use comux::args::CliArgs;
use comux::commands::run;

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
