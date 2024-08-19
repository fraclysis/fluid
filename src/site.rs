pub fn build_site() {
    let mut config = load_config("config.yaml");

    let mut theme: String;
    if let Some(l) = config.get("_theme") {
        if l.is_string() {
            theme = l.as_string().unwrap().clone();
        } else {
            eprintln!("`_theme` must be a string. {l:?}");
            process::exit(1);
        }
    } else {
        eprintln!("`config.yaml` must contain `_theme` variable.");
        process::exit(1);
    }

    let mut theme_path = PathBuf::from("themes");
    theme_path.push(theme);

    let mut src: Option<String> = None;
    if let Some(l) = HashMap::get_mut(&mut config, "_source") {
        l.with_string(|s| src = Some(s.to_owned()));
    }

    let src = src.unwrap_or_else(|| String::from("content"));

    let mut out: Option<String> = None;
    if let Some(l) = HashMap::get_mut(&mut config, "_destination") {
        l.with_string(|s| out = Some(s.to_owned()));
    }

    let out = out.unwrap_or_else(|| String::from("out"));

    build_tree();
}

fn load_config(path: &str) -> MutRc<Object> {
    let content = read_to_string_fatal(path);
    let yaml = yaml_rust::Yaml::from_str(&content);
    let liquid = yaml_to_liquid(yaml);
    liquid.as_object().unwrap()
}

fn read_to_string_fatal<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(&path).unwrap_or_else(|e| {
        let path = path.as_ref().to_str().unwrap_or_else(|| "<ENCODING_ERROR>");

        eprintln!("Failed to read file {path}. {e}");
        process::exit(1)
    })
}

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process,
};

use crate::{
    liquid::{Liquid, MutRc, Object},
    parser::{yaml_to_liquid, LiquidState},
};
