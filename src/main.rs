use chrono::Local;
use clap::{Parser, Subcommand};
use env_logger::Builder;
use log::{error, LevelFilter};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use std::io::Write;
use std::process;
use std::{env, io};

use mdbook_classy::preprocessor::Classy;

/// mdbook preprocessor to add support for admonitions
#[derive(clap::Parser)]
#[command(author, version, long_about = None)]
#[command(name = "classy")]
#[command(
    about = "A mdbook preprocessor that recognizes kramdown style paragraph class annotation."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check whether a renderer is supported by this preprocessor
    Supports { renderer: String },
}

/// Housekeeping:
/// 1. Check compatibility between preprocessor and mdbook
/// 2. deserialize, run the transformation, and reserialize.
fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<(), Error> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    if ctx.mdbook_version != mdbook::MDBOOK_VERSION {
        // We should probably use the `semver` crate to check compatibility
        // here...
        error!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

/// Check to see if we support the processor (classy only supports html right now)
fn handle_supports(pre: &dyn Preprocessor, renderer: &str) -> ! {
    let supported = pre.supports_renderer(renderer);

    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

fn init_logger() {
    let mut builder = Builder::new();

    builder.format(|formatter, record| {
        writeln!(
            formatter,
            "{} [{}] ({}): {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.target(),
            record.args()
        )
    });

    if let Ok(var) = env::var("RUST_LOG") {
        builder.parse_filters(&var);
    } else {
        // if no RUST_LOG provided, default to logging at the Info level
        builder.filter(None, LevelFilter::Info);
        // Filter extraneous html5ever not-implemented messages
        builder.filter(Some("html5ever"), LevelFilter::Error);
    }

    builder.init();
}

fn main() {
    init_logger();

    // 1. Define command interface, requiring renderer to be specified.
    let args = Cli::parse();

    // 2. Instantiate the preprocessor.
    let preprocessor = Classy::new();

    match args.command {
        None => {
            if let Err(e) = handle_preprocessing(&preprocessor) {
                error!("{}", e);
                process::exit(1);
            }
        }
        Some(Commands::Supports { renderer }) => {
            handle_supports(&preprocessor, &renderer);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    
    #[test]
    fn verify_cli() {
        Cli::command().debug_assert()
    }
}
