fn main() {
    let args = Args::parse();

    if let Some(cd) = args.cd {
        env::set_current_dir(&cd).exit(cd);
    }

    match args.command {
        Cmd::Serve => site::serve(),
        Cmd::Build => site::build(),
        Cmd::New { name } => init::init_in(&name),
        Cmd::Init => init::init(),
        Cmd::Post { name: _, path: _ } => todo!(),
    }
}

#[derive(clap::Subcommand)]
enum Cmd {
    Serve,
    Build,
    New { name: String },
    Init,
    Post { name: String, path: Option<String> },
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short = 'C')]
    cd: Option<String>,

    #[command(subcommand)]
    command: Cmd,
}

use std::env;

use clap::Parser;
use helper::IoError;

pub mod config;
pub mod helper;
mod init;
pub mod liquid;
pub mod markdown;
pub mod page;
pub mod parser;
pub mod plugins;
pub mod site;
