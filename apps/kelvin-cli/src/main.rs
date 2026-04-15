mod bare;
mod cli;
mod cmd;
mod env;
mod keys;
mod paths;
mod proc;
mod tty;

use clap::Parser;

use cli::{Cli, Commands};

fn main() {
    // Load dotenv before parsing args so env vars are available to clap,
    // and before initializing the Tokio runtime.
    env::load_dotenv();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime");

    runtime.block_on(async_main());
}

async fn async_main() {
    let cli = Cli::parse();

    let result = match cli.command {
        None => bare::run().await.map(|_| ()),
        Some(Commands::Start(args)) => cmd::start::run(args),
        Some(Commands::Stop) => cmd::stop::run(),
        Some(Commands::Tui(args)) => cmd::tui::run(args),
        Some(Commands::Gateway { sub }) => cmd::gateway::run(sub),
        Some(Commands::Memory { sub }) => cmd::memory::run(sub),
        Some(Commands::Plugin { sub }) | Some(Commands::Kpm { sub }) => cmd::plugin::run(sub),
        Some(Commands::Init(args)) => cmd::init::run(args),
        Some(Commands::Medkit(args)) => cmd::medkit::run(args),
        Some(Commands::Doctor) => cmd::doctor::run(),
        Some(Commands::Service { sub }) => cmd::service::run(sub),
        Some(Commands::Completions(args)) => cmd::completions::run(args),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
