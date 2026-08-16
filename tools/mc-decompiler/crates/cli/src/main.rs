mod args;
mod commands;

use clap::Parser;

use args::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        args::Command::Setup => commands::setup::run(),
        args::Command::Decompile {
            version,
            jar,
            force,
        } => commands::decompile::run(&version, jar.as_deref(), force),
        args::Command::Download { version, output } => {
            commands::download::run(&version, output.as_deref())
        }
        args::Command::List => commands::list::run(),
        args::Command::Diff { from, to, class } => {
            commands::diff::run(&from, &to, class.as_deref())
        }
        args::Command::Search { query, version } => {
            commands::search::run(&query, version.as_deref())
        }
        args::Command::Show { version, class } => commands::show::run(&version, &class),
    }
}
