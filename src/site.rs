pub fn build() {
    let config = Config::new("config.yaml");
    let layouts = load_layouts(&config.theme);

    let root = load_pages("site");
}

pub fn serve() {
    unimplemented!()
}

fn load_layouts(name: &str) -> HashMap<String, String> {
    let mut layouts = HashMap::new();

    let mut path = PathBuf::from("themes");
    path.push(name);
    path.push("layouts");

    for e in fs::read_dir(&path).exit(path).map(|e| e.unwrap()) {
        let ep = e.path();

        if e.file_type().exit(&ep).is_dir() {
            continue;
        }

        if let Some(name) = ep.file_name() {
            let name = name.to_str().unwrap();
            let content = fs::read_to_string(&ep).exit(&ep);

            layouts.insert(name.to_string(), content);
        }
    }

    layouts
}

fn load_pages(path: &str, parent: &mut Option<Ru<Object>>) {
    let mut directory: Ru<Object> = Ru::default();

    directory.insert(Folder::FILES.to_string(), Liquid::default_array());
    directory.insert(Folder::FOLDERS.to_string(), Liquid::default_array());

    if let Some(parent) = parent {
        parent.get_mut(Folder::FOLDERS).unwrap().with_array(|a| {
            a.push(directory.into());
        });
    }

    for e in fs::read_dir(path).exit(path).map(|e| e.exit(path)) {
        let p = e.path();

        if e.file_type().exit(&p).is_dir() {
        } else {
        }
    }

    todo!()
}

use std::{collections::HashMap, fs, path::PathBuf};

use crate::{
    config::Config,
    helper::{IoError, Ru},
    liquid::{Liquid, Object},
    page::Folder,
};
