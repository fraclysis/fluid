mod markdown;
mod plugins;

use std::{
    collections::HashMap,
    fmt,
    fs::{self, remove_dir_all, remove_file, DirEntry},
    io,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

use yaml_rust::Yaml;

use crate::plugins::Plugins;

type Object = HashMap<String, Liquid>;
type Array = Vec<Liquid>;

const SITE: &str = "site";
const LAYOUTS_FOLDER: &str = "layouts";

const PAGE_CONTENT: &str = "contents";
const PAGE_PARENT: &str = "parent";
const PAGE_PATH: &str = "path";
const PAGE_OUT_FOLDER: &str = "out";
const PAGE_LAYOUT: &str = "layout";
const PAGE_FRONT_MATTER_OFFSET: &str = "front_matter_offset";

const FOLDER_FOLDERS: &str = "folders";
const FOLDER_FILES: &str = "files";

#[derive(Clone)]
pub enum Liquid {
    String(String),
    Int(i32),
    Bool(bool),

    Object(Rc<Object>),
    WeakObject(Weak<Object>),

    Array(Rc<Array>),
    WeakArray(Weak<Array>),

    Nil,
}

impl fmt::Debug for Liquid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(arg0) => write!(f, "{:#?}", arg0),
            Self::Int(arg0) => write!(f, "{:#?}", arg0),
            Self::Bool(arg0) => write!(f, "{:#?}", arg0),
            Self::Object(arg0) => write!(f, "{:#?}", arg0),
            Self::WeakObject(arg0) => write!(f, "{:#?}", arg0),
            Self::Array(arg0) => write!(f, "{:#?}", arg0),
            Self::WeakArray(arg0) => write!(f, "{:#?}", arg0),
            Self::Nil => write!(f, "Nil"),
        }
    }
}

impl Liquid {
    pub fn as_str(&self) -> &str {
        match self {
            Liquid::String(s) => s,
            _ => "",
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Liquid::String(s) => s.clone(),
            Liquid::Int(i) => format!("{}", i),
            Liquid::Object(o) => format!("{:#?}", o),
            Liquid::Array(a) => format!("{:#?}", a),
            Liquid::Nil => "".to_string(),
            Liquid::Bool(b) => match b {
                true => "true".to_string(),
                false => "false".to_string(),
            },
            Liquid::WeakObject(o) => match o.upgrade() {
                Some(o) => format!("{:#?}", o),
                None => "".to_string(),
            },
            Liquid::WeakArray(_) => todo!(),
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Liquid::Int(i) => *i,
            _ => todo!(),
        }
    }

    fn as_hash(&self) -> Rc<Object> {
        match self {
            Liquid::Object(o) => o.clone(),
            _ => panic!(),
        }
    }

    fn as_vec(&self) -> Rc<Array> {
        match self {
            Liquid::Array(o) => o.clone(),
            Liquid::Nil => Rc::new(Vec::new()),
            _ => panic!(),
        }
    }

    pub fn get_prop(&self, in_prop: &str) -> Result<Liquid, String> {
        use Liquid::*;

        let props: Vec<&str> = in_prop.split('.').map(|p| p.trim()).collect();
        let tags = Plugins::get();

        let mut liquid = self.clone();

        for prop in props {
            if tags.functions.contains_key(prop) {
                liquid = tags.functions.get(prop).expect("Checked")(&liquid)?;
                continue;
            }

            match liquid {
                Object(o) => {
                    let o = cast_mut(o.as_ref());
                    match o.get(prop) {
                        Some(l) => liquid = l.clone(),
                        None => {
                            return Err(format!(
                                "Object does not contains the prop: {prop}:{in_prop}"
                            ))
                        }
                    }
                }

                Array(a) => {
                    let a = cast_mut(a.as_ref());

                    match prop.parse::<usize>() {
                        Ok(i) => match a.get(i) {
                            Some(l) => liquid = l.clone(),
                            None => {
                                return Err(format!(
                                    "Array size is {arr_len} requested index is \"{i}\" : \"{in_prop}\"",
                                    arr_len = a.len()
                                ))
                            }
                        },
                        Err(e) => {
                            return Err(format!(
                                "Failed to parse prop \"{prop}\" : \"{in_prop}\" as usize: {e}"
                            ))
                        }
                    }
                }

                WeakObject(o) => match o.upgrade() {
                    Some(o) => {
                        let o = cast_mut(o.as_ref());
                        match o.get(prop) {
                            Some(l) => liquid = l.clone(),
                            None => {
                                return Err(format!(
                                    "Object does not contains the prop: {prop}:{in_prop}"
                                ))
                            }
                        }
                    }
                    None => return Err(format!("Prop failed upgrade \"{prop}\" : \"{in_prop}\"")),
                },

                WeakArray(a) => match a.upgrade() {
                    Some(a) => {
                        let a = cast_mut(a.as_ref());

                        match prop.parse::<usize>() {
                            Ok(i) => match a.get(i) {
                                Some(l) => liquid = l.clone(),
                                None => return Err(format!(
                                    "Array size is {arr_len} requested index is \"{i}\" : \"{in_prop}\"",
                                    arr_len = a.len()
                                )),
                            },
                            Err(e) =>   return Err(format!(
                                "Failed to parse prop \"{prop}\" : \"{in_prop}\" as usize: {e}"
                            )),
                        }
                    }
                    None => return Err(format!("Prop failed upgrade \"{prop}\" : \"{in_prop}\"")),
                },
                String(_) => {
                    return Err(format!(
                        "Can not index into String \"{prop}\" : \"{in_prop}\""
                    ))
                }
                Int(_) => return Err(format!("Can not index into Int \"{prop}\" : \"{in_prop}\"")),
                Bool(_) => {
                    return Err(format!(
                        "Can not index into Bool \"{prop}\" : \"{in_prop}\""
                    ))
                }
                Nil => return Err(format!("Can not index into Nil \"{prop}\" : \"{in_prop}\"")),
            }
        }

        Ok(liquid)
    }
}

fn main() {
    Plugins::init();
    Plugins::show();

    let working_dir = PathBuf::from("site");

    fn create_folder_starter<P: AsRef<Path>>(working_dir: P) -> Result<Rc<Object>, io::Error> {
        let folders = Rc::new(Vec::new());
        let files = Rc::new(Vec::new());

        let mut this = HashMap::new();
        this.insert(FOLDER_FOLDERS.to_string(), Liquid::Array(folders));
        this.insert(FOLDER_FILES.to_string(), Liquid::Array(files));

        let this = Rc::new(this);

        for e in fs::read_dir(working_dir)? {
            fn failable(
                e: Result<DirEntry, io::Error>,
                parent: Rc<Object>,
            ) -> Result<(), io::Error> {
                let e = e?;
                let path = e.path();

                match e.file_type()?.is_dir() {
                    true => create_folder(path, parent),
                    false => create_file(path, parent),
                }
            }

            match failable(e, this.clone()) {
                Ok(_) => (),
                Err(e) => {
                    eprint!("\n\n{}\n\n", e);
                    continue;
                }
            }
        }

        Ok(this)
    }

    let parent = create_folder_starter(working_dir).unwrap();
    let parent: &mut HashMap<String, Liquid> = cast_mut(parent.as_ref());

    clear_dir("out").unwrap_or_else(|e| eprintln!("Could not clean the \"out\" folder. {e}"));

    let assets_path = PathBuf::from("assets");
    if !assets_path.exists() {
        fs::create_dir(&assets_path)
            .unwrap_or_else(|e| eprintln!("Could not create \"assets\" folder. {e}"));
    }

    generate_assets(assets_path)
        .unwrap_or_else(|e| eprintln!("Some error in \"assets\" folder. {e}"));
    parse_nodes(parent);

    Plugins::terminate();

    fn parse_nodes(node: &mut HashMap<String, Liquid>) {
        let folders = node
            .get(FOLDER_FOLDERS)
            .expect("Object does not contain argument.")
            .as_vec();
        let files = node
            .get(FOLDER_FILES)
            .expect("Object does not contain argument.")
            .as_vec();

        let folders: &mut Vec<Liquid> = cast_mut(folders.as_ref());
        let files = cast_mut(files.as_ref());

        for folder_liq in folders {
            let file = folder_liq.as_hash();
            let file = cast_mut(file.as_ref());

            parse_nodes(file)
        }

        for file_liq in files {
            fn parse_file(file_liq: &mut Liquid) -> Result<(), io::Error> {
                let file = file_liq.as_hash();
                let file = cast_mut(file.as_ref());

                let content = file.get(PAGE_CONTENT).unwrap().as_str();
                let path = file.get(PAGE_PATH).unwrap().as_str();

                let out_path_rel = &path[SITE.len() + 1..];
                let mut out_path = PathBuf::from(PAGE_OUT_FOLDER);
                out_path.push(out_path_rel);

                {
                    if let Some(out_str) = out_path.to_str() {
                        let out_str = out_str.replace(".md", ".html");
                        out_path = PathBuf::from(out_str)
                    }
                }

                let mut offset = 0;

                let layout = match file.get(PAGE_LAYOUT) {
                    Some(s) => s.as_str(),
                    None => {
                        offset = file
                            .get(PAGE_FRONT_MATTER_OFFSET)
                            .expect("Created by application.")
                            .as_int();

                        if offset != 0 {
                            offset += 3;
                        }

                        "paste_in"
                    } // TODO:(frac) get rid of str
                };

                fn get_layout(file_name: &str) -> Option<&String> {
                    static mut LAYOUTS: Option<HashMap<String, String>> = None;

                    unsafe {
                        match &mut LAYOUTS {
                            Some(l) => l.get(file_name),
                            None => {
                                let mut layouts = HashMap::new();

                                let read_dir = match fs::read_dir(LAYOUTS_FOLDER) {
                                    Ok(read_dir) => read_dir,
                                    Err(e) => match e.kind() {
                                        io::ErrorKind::NotFound => {
                                            fs::create_dir(LAYOUTS_FOLDER)
                                                .expect("Failed to create layouts");
                                            LAYOUTS = Some(HashMap::new());
                                            return None;
                                        }
                                        _ => unimplemented!(),
                                    },
                                };

                                for e in read_dir {
                                    let e = e.unwrap_or_else(|e| panic!("{e}"));

                                    let name = e.file_name().to_str().unwrap().to_string();
                                    let contents = fs::read_to_string(e.path()).unwrap();

                                    layouts.insert(name, contents);
                                }

                                LAYOUTS = Some(layouts);
                                match &mut LAYOUTS {
                                    Some(l) => l.get(file_name),
                                    None => std::hint::unreachable_unchecked(),
                                }
                            }
                        }
                    }
                }

                fn walk_write(path: PathBuf, content: &str) -> Result<(), io::Error> {
                    if let Err(e) = fs::write(&path, content) {
                        match e.kind() {
                            io::ErrorKind::NotFound => {
                                let mut folder_path = path.clone();
                                folder_path.pop();
                                fs::DirBuilder::new().recursive(true).create(folder_path)?;
                                return walk_write(path, content);
                            }
                            _ => return Err(e),
                        }
                    }
                    Ok(())
                }

                match layout {
                    "paste_in" => match parse(content, file_liq, offset as _) {
                        Ok(parsed) => walk_write(out_path, &parsed),
                        Err(e) => Err(e.into()),
                    },
                    any => match get_layout(any) {
                        Some(layout) => match parse(layout, file_liq, offset as _) {
                            Ok(parsed) => walk_write(out_path, &parsed),
                            Err(mut e) => {
                                e.error_path = format!("layouts/{any}");
                                Err(e.into())
                            }
                        },
                        None => Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Layout `{any}` could not found."),
                        )),
                    },
                }
            }

            if let Err(e) = parse_file(file_liq) {
                // ? Main Error Point
                eprintln!("{e}");
            }
        }
    }

    fn clear_dir<P: AsRef<Path>>(path: P) -> Result<(), io::Error> {
        for e in fs::read_dir(path)? {
            let e = e?;

            match e.file_type()?.is_dir() {
                true => remove_dir_all(e.path())?,
                false => remove_file(e.path())?,
            }
        }
        Ok(())
    }

    // ! If we get absolute path this function will fail
    fn generate_assets<P: AsRef<Path>>(assets_dir: P) -> Result<(), io::Error> {
        let mut output_dir = PathBuf::from("out");
        output_dir.push(&assets_dir);
        fs::create_dir(output_dir)?;

        for e in fs::read_dir(assets_dir)? {
            fn failable(e: Result<DirEntry, io::Error>) -> Result<(), io::Error> {
                let e = e?;

                match e.file_type()?.is_dir() {
                    true => generate_assets(e.path()),
                    false => {
                        let path = e.path();
                        let mut out = PathBuf::from("out");
                        out.push(&path);

                        if let Err(e) = fs::copy(path, out) {
                            return Err(e);
                        };

                        Ok(())
                    }
                }
            }

            match failable(e) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("\n\n{e}\n\n") // ? Main Eprint
                }
            }
        }

        Ok(())
    }
}

fn get_front_matter_offset(text: &str) -> isize {
    let text_start = text.as_ptr();
    let mut text_end = text.as_ptr();

    let mut contains_front_matter = false;

    for line in text.lines() {
        if contains_front_matter {
            if line.trim_end() == "---" {
                text_end = line.as_ptr();
                break;
            }
            continue;
        }

        if line.trim_end() == "---" {
            contains_front_matter = true;
        }
    }

    unsafe { text_end.offset_from(text_start) }
}

fn create_folder<P: AsRef<Path>>(path: P, upper_folder: Rc<Object>) -> Result<(), io::Error> {
    let folders = Rc::new(Vec::new());
    let files = Rc::new(Vec::new());

    let mut this = HashMap::new();
    this.insert(FOLDER_FOLDERS.to_string(), Liquid::Array(folders));
    this.insert(FOLDER_FILES.to_string(), Liquid::Array(files));
    this.insert(
        PAGE_PARENT.to_string(),
        Liquid::WeakObject(Rc::downgrade(&upper_folder)),
    );

    let this = Rc::new(this);

    {
        cast_mut(
            upper_folder
                .as_ref()
                .get(FOLDER_FOLDERS)
                .expect("folders not found")
                .as_vec()
                .as_ref(),
        )
        .push(Liquid::Object(this.clone()));
    }

    for e in fs::read_dir(path)? {
        fn failable(e: Result<DirEntry, io::Error>, parent: Rc<Object>) -> Result<(), io::Error> {
            let e = e?;
            let path = e.path();

            match e.file_type()?.is_dir() {
                true => create_folder(path, parent),
                false => create_file(path, parent),
            }
        }

        match failable(e, this.clone()) {
            Ok(_) => (),
            Err(e) => {
                eprint!("{}", e);
                continue;
            }
        }
    }

    Ok(())
}

fn create_file<P: AsRef<Path>>(path: P, parent: Rc<Object>) -> Result<(), io::Error> {
    let content = fs::read_to_string(&path)?;

    let path_str = match path.as_ref().to_str() {
        Some(s) => s,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Path is not utf8",
            ));
        }
    };

    let front_matter_finish_index = get_front_matter_offset(&content) as usize;
    let yaml_source = {
        let non_trimmed_yaml_source = &content[..front_matter_finish_index];

        if non_trimmed_yaml_source.len() == 0 {
            non_trimmed_yaml_source
        } else {
            &non_trimmed_yaml_source[3..non_trimmed_yaml_source.len()]
        }
    };

    fn yaml_to_liquid(yaml: Yaml) -> Liquid {
        match yaml {
            Yaml::Real(s) => Liquid::String(s),
            Yaml::Integer(i) => Liquid::Int(i as i32),
            Yaml::String(s) => Liquid::String(s),
            Yaml::Boolean(b) => Liquid::Bool(b),
            Yaml::Array(a) => {
                let mut l = Vec::new();
                for y in a {
                    l.push(yaml_to_liquid(y));
                }
                Liquid::Array(Rc::new(l))
            }
            Yaml::Hash(h) => {
                let mut hash = HashMap::new();

                for (k, v) in h {
                    hash.insert(k.as_str().unwrap().to_string(), yaml_to_liquid(v));
                }

                Liquid::Object(Rc::new(hash))
            }
            Yaml::Alias(a) => Liquid::Int(a as i32),
            Yaml::Null => Liquid::Nil,
            Yaml::BadValue => Liquid::Nil,
        }
    }

    let front_matter = {
        let mut yaml = match yaml_rust::YamlLoader::load_from_str(yaml_source) {
            Ok(y) => y,
            Err(e) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        };

        match yaml.pop() {
            Some(yaml) => yaml_to_liquid(yaml),
            None => Liquid::Object(Rc::new(HashMap::new())),
        }
    };

    {
        let front_matter_rc = front_matter.as_hash();
        let front_matter = cast_mut(front_matter_rc.as_ref());

        front_matter.insert(PAGE_CONTENT.to_string(), Liquid::String(content));
        front_matter.insert(PAGE_PATH.to_string(), Liquid::String(path_str.to_string()));
        front_matter.insert(
            PAGE_FRONT_MATTER_OFFSET.to_string(),
            Liquid::Int(front_matter_finish_index as _),
        );

        front_matter.insert(
            PAGE_PARENT.to_string(),
            Liquid::WeakObject(Rc::downgrade(&parent)),
        );
    }

    {
        cast_mut(
            parent
                .as_ref()
                .get(FOLDER_FILES)
                .expect("folders not found")
                .as_vec()
                .as_ref(),
        )
        .push(front_matter);
    }

    Ok(())
}

#[allow(clippy::cast_ref_to_mut)]
#[allow(clippy::mut_from_ref)]
fn cast_mut<T>(val: &T) -> &mut T {
    unsafe { &mut *(val as *const T as *mut T) }
}

#[derive(Debug)]
struct ParseError<'a> {
    content: &'a str,
    tag_start: usize,
    tag_end: usize,
    error_message: String,
    error_path: String,
}

impl<'a> std::fmt::Display for ParseError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, column) = get_line_and_column(self.content, self.tag_start);
        let tag = &self.content[self.tag_start..self.tag_end];
        let message = &self.error_message;
        let path = &self.error_path;
        write!(
            f,
            "Error occured on {tag} in \"{path}:{line}:{column}\" with message \"{message}\"."
        )
    }
}

impl<'a> From<ParseError<'a>> for io::Error {
    fn from(value: ParseError) -> Self {
        Self::new(io::ErrorKind::InvalidData, format!("{value}"))
    }
}

lazy_static::lazy_static!(
  pub static ref REG: regex::Regex = regex::Regex::new(r"\{(\{|%)[^\{,\n}]*(}|%)}").unwrap();
);

enum BlockState<'a> {
    // Opened(regex::Match<'a>, LiquidTag<'a>, String),
    Closed,

    Opened {
        block_opener: regex::Match<'a>,
        block_opener_tag: &'a str,
        block_opener_plugin: &'a str,
        block_ender: String,
    },
}

fn parse<'a>(
    to_parse: &'a str,
    object: &mut Liquid,
    skip_offset: usize,
) -> Result<String, ParseError<'a>> {
    let liquid_tags = Plugins::get();
    let mut parsed: Vec<String> = Vec::new();

    let mut state = BlockState::Closed;
    let mut last_found_end = skip_offset;
    let mut request_trim_start = false;

    let mut same_block_recursion = 0;

    for found in REG.find_iter(to_parse) {
        let non_trimmed_tag = found.as_str();
        let len = non_trimmed_tag.len();

        let trim_start = non_trimmed_tag[2..3].contains('-');
        let trim_end = non_trimmed_tag[len - 3..len - 2].contains('-');

        let tag = {
            let start_offset = trim_start as usize;
            let end_offset = trim_end as usize;

            non_trimmed_tag[2 + start_offset..len - (2 + end_offset)].trim()
        };
        let plugin_name = tag.split(' ').next().unwrap().trim(); // ! error hanle if the tag is exixts

        match state {
            BlockState::Closed => {
                // ? SYNC:(non_mached_push)
                {
                    let mut non_mached_text = &to_parse[last_found_end..found.start()];
                    last_found_end = found.end();

                    if request_trim_start {
                        non_mached_text = non_mached_text.trim_start();
                    }

                    if trim_end {
                        non_mached_text = non_mached_text.trim_end();
                    }

                    parsed.push(non_mached_text.to_string());
                    request_trim_start = trim_start;
                }

                match &non_trimmed_tag[..2] {
                    "{{" => {
                        let mut target_object = match object.get_prop(plugin_name) {
                            Ok(s) => s,
                            Err(e) => {
                                return Err(ParseError {
                                    content: to_parse,
                                    tag_start: found.start(),
                                    tag_end: found.end(),
                                    error_message: e,
                                    error_path: get_object_path(object),
                                })
                            }
                        };

                        let first_pipe = tag.find('|').unwrap_or_default();

                        let filters: Vec<&str>;
                        if first_pipe == 0 {
                            filters = Vec::new()
                        } else {
                            filters = tag[first_pipe + 1..].split('|').map(|s| s.trim()).collect();
                        }

                        for filter in filters {
                            if let Some(filter_proc) = liquid_tags.filters.get(filter) {
                                match filter_proc(tag, &mut target_object, object) {
                                    Ok(success) => target_object = success,
                                    Err(_) => todo!(), // ! error handling on procs
                                };
                            } else {
                                return Err(ParseError {
                                    content: to_parse,
                                    tag_start: found.start(),
                                    tag_end: found.end(),
                                    error_message: format!("Filter {filter} does not exists."),
                                    error_path: get_object_path(object),
                                });
                            }
                            // ! Handle plugin proc
                        }

                        // ? SYNC:(output_parser)
                        {
                            let output = target_object.as_string();

                            let mut output_trimable = output.as_str();

                            if request_trim_start {
                                output_trimable = output_trimable.trim_start();
                            }

                            if trim_end {
                                output_trimable = output_trimable.trim_end();
                            }

                            parsed.push(output_trimable.to_string());

                            request_trim_start = trim_start;
                        }
                    }
                    "{%" => {
                        if liquid_tags.blocks.contains_key(plugin_name) {
                            state = BlockState::Opened {
                                block_opener: found,
                                block_opener_tag: tag,
                                block_opener_plugin: plugin_name,
                                block_ender: "end".to_string() + plugin_name,
                            };
                            continue;
                        }

                        if let Some(tag_proc) = liquid_tags.tags.get(plugin_name) {
                            let output = match tag_proc(tag, object) {
                                Ok(success) => success,
                                Err(e) => {
                                    return Err(ParseError {
                                        content: to_parse,
                                        tag_start: found.start(),
                                        tag_end: found.end(),
                                        error_message: e,
                                        error_path: get_object_path(object),
                                    })
                                } // ! Handle error
                            };

                            // ? SYNC:(output_parser)
                            {
                                let output = output.as_string();

                                let mut output_trimable = output.as_str();

                                if request_trim_start {
                                    output_trimable = output_trimable.trim_start();
                                }

                                if trim_end {
                                    output_trimable = output_trimable.trim_end();
                                }

                                parsed.push(output_trimable.to_string());

                                request_trim_start = trim_start;
                            }

                            continue;
                        }

                        return Err(ParseError {
                            content: to_parse,
                            tag_start: found.start(),
                            tag_end: found.end(),
                            error_message: format!("\"{plugin_name}\" does not exist in plugins."),
                            error_path: get_object_path(object),
                        });
                    }
                    _ => unreachable!(),
                }
            }
            BlockState::Opened {
                block_opener,
                block_opener_tag,
                block_opener_plugin,
                block_ender,
            } => {
                if plugin_name == block_ender {
                    if same_block_recursion > 0 {
                        same_block_recursion -= 1;
                        state = BlockState::Opened {
                            block_opener,
                            block_opener_tag,
                            block_opener_plugin,
                            block_ender,
                        };
                        continue;
                    }

                    state = BlockState::Closed;
                    last_found_end = found.end();

                    let block_text = &to_parse[block_opener.end()..found.start()];

                    let plugin_proc = liquid_tags
                        .blocks
                        .get(block_opener_plugin)
                        .expect("Checked while opening block.");

                    let output = match plugin_proc(block_opener_tag, block_text, object) {
                        Ok(success) => success,
                        Err(e) => {
                            return Err(ParseError {
                                content: to_parse,
                                tag_start: found.start(),
                                tag_end: found.end(),
                                error_message: e,
                                error_path: get_object_path(object),
                            })
                        }
                    };

                    // ? SYNC:(output_parser)
                    {
                        let output = output.as_string();

                        let mut output_trimable = output.as_str();

                        if request_trim_start {
                            output_trimable = output_trimable.trim_start();
                        }

                        if trim_end {
                            output_trimable = output_trimable.trim_end();
                        }

                        parsed.push(output_trimable.to_string());

                        request_trim_start = trim_start;
                    }
                } else {
                    if block_opener_plugin == plugin_name {
                        same_block_recursion += 1;
                    }
                    state = BlockState::Opened {
                        block_opener,
                        block_opener_tag,
                        block_opener_plugin,
                        block_ender,
                    }
                }
            }
        }
    }

    if request_trim_start {
        parsed.push(to_parse[last_found_end..].trim_start().to_string()); // remaining text
    } else {
        parsed.push(to_parse[last_found_end..].to_string()); // remaining text
    }

    Ok(parsed.join(""))
}

fn get_object_path(object: &mut Liquid) -> String {
    match object.get_prop("path") {
        Ok(p) => p.as_string(),
        Err(_) => String::new(),
    }
}

pub fn get_line_and_column(text: &str, match_start: usize) -> (usize, usize) {
    let lines = match text.get(..match_start + 1) {
        Some(s) => s.lines(),
        None => text[..match_start].lines(),
    };

    let line = lines.clone().count();
    let column = lines.last().unwrap().len();
    (line, column)
}
