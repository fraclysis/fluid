pub mod liquid;
#[cfg(feature = "live")]
mod live_reload;
mod markdown;
mod new;
pub mod parser;
pub mod plugins;

#[cfg(feature = "live")]
mod watcher;

use std::{
    cell::UnsafeCell,
    io::{Error, ErrorKind},
    thread::sleep,
    time::{Duration, Instant},
};

use clap::Parser;
#[cfg(feature = "live")]
use live_reload::live_reload_thread;
#[cfg(feature = "live")]
use watcher::{Watcher, LAYOUTS_FOLDER_STATUS_ID};

use crate::{
    liquid::Liquid,
    parser::{site_parse, yaml_to_liquid, LAYOUTS},
    plugins::Plugins,
};

#[derive(clap::Subcommand)]
enum Cmd {
    Serve,
    Build,
    New,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short = 'C')]
    cd: Option<String>,
    #[command(subcommand)]
    command: Cmd,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    if let Some(cd) = args.cd {
        std::env::set_current_dir(cd).unwrap();
    }

    let config = Config::new()?;

    let plugins = Plugins::new();
    plugins.show();
    reload_site(&plugins, &config, false);

    match args.command {
        Cmd::New => {}
        Cmd::Build => {}
        Cmd::Serve => {}
    }

    Ok(())
}

fn reload_site(plugins: &Plugins, config: &Config, reset_layouts: bool) {
    if reset_layouts {
        if let Some(layouts) = unsafe { &mut LAYOUTS } {
            layouts.clear();
        }
    }
    let time = Instant::now();
    site_parse(plugins).warn();
    let elapsed = time.elapsed();

    println!(
        "Updated http://{}.{}.{}.{}:{} in {:?}",
        config.host[0], config.host[1], config.host[2], config.host[3], config.port, elapsed
    );
}

pub struct Config {
    pub host: [u8; 4],
    pub port: u16,
    pub other: Liquid,
}

impl Config {
    fn new() -> Result<Self, Error> {
        let file = std::fs::read_to_string("fluid.yaml")?;
        let mut file = match yaml_rust::YamlLoader::load_from_str(&file) {
            Ok(y) => y,
            Err(e) => return Err(Error::new(ErrorKind::InvalidData, e)),
        };

        let config = match file.pop() {
            Some(y) => yaml_to_liquid(y),
            None => ().into(),
        };

        let mut host = [127, 0, 0, 1];
        let mut port = 3000;

        if let Some(config) = config.as_object() {
            if let Some(s) = config.get("host") {
                let mut i = 0;
                if let Some(s) = s.as_string() {
                    for n in s.split(".") {
                        if i > 4 {
                            return Err(Error::new(ErrorKind::InvalidData, "Bad host address."));
                        }

                        if let Ok(a) = n.parse::<i32>() {
                            host[i] = a as u8;
                        }
                        i += 1;
                    }
                }
            }

            if let Some(i) = &config.get("port") {
                if let Some(i) = i.as_int() {
                    port = i as u16;
                }
            }
        }

        Ok(Self {
            host,
            port,
            other: config,
        })
    }
}

pub mod config;
pub mod helper;
