use std::{
    cell::UnsafeCell,
    collections::HashMap,
    fmt::{self, Display},
    fs::{self, remove_dir_all, remove_file, DirEntry},
    io::{self, Error},
    path::{Path, PathBuf},
    rc::Rc,
};

use yaml_rust::Yaml;

use crate::{
    helper::IntoIoResult,
    liquid::{Liquid, LiquidInner, MutRc, Object, OptionToResult},
    plugins::Plugins,
};

pub type LiquidResult = Result<Liquid, ParseError>;

pub struct LiquidState<'a> {
    pub plugins: &'a Plugins,
    pub file_path: &'a str,
    pub current_line: usize,
    pub current_column: usize,
}

impl<'a> LiquidState<'a> {
    pub fn get_key(&self, liquid: &Liquid, in_prop: &str) -> Result<Liquid, ParseError> {
        let keys: Vec<&str> = in_prop.split('.').map(|p| p.trim()).collect();

        let mut temp = liquid.clone();

        for key in keys {
            if let Some(f) = self.plugins.functions.get(key) {
                temp = f(self, &temp)?;

                continue;
            }

            if temp.is_object() {
                if let Some(value) = temp.as_object().unwrap().get(key) {
                    temp = value.clone();
                } else {
                    return Err(format!("Optional::None").into());
                }

                continue;
            }

            if temp.is_array() {
                let index = match key.parse::<usize>() {
                    Ok(v) => v,
                    Err(e) => return Err(format!("{e}").into()),
                };
                if let Some(value) = temp.as_array().unwrap().get(index) {
                    temp = value.clone();
                } else {
                    return Err(format!("Optional::None").into());
                }

                continue;
            }

            return Err(format!("Cannot Index Into").into());
        }

        Ok(temp)
    }
}

pub static mut LAYOUTS: Option<HashMap<String, String>> = None;

const SITE: &str = "site";
const LAYOUTS_FOLDER: &str = "layouts";

pub const PAGE_CONTENT: &str = "contents";
const PAGE_PARENT: &str = "parent";
const PAGE_PATH: &str = "path";
const PAGE_OUT_FOLDER: &str = "out";
const PAGE_LAYOUT: &str = "layout";
pub const PAGE_FRONT_MATTER_OFFSET: &str = "front_matter_offset";

const FOLDER_FIELDS_FOLDER: &str = "folders";
const FOLDER_FIELDS_FILES: &str = "files";

pub fn site_parse(liquid_tags: &Plugins) -> Result<(), Error> {
    fn create_folder_starter<P: AsRef<Path>>(working_dir: P) -> Result<Liquid, io::Error> {
        let base = Liquid::default_object();

        let working_dir: &Path = working_dir.as_ref();

        base.with_object(|object| {
            object.insert(FOLDER_FIELDS_FOLDER.to_string(), Liquid::default_array());
            object.insert(FOLDER_FIELDS_FILES.to_string(), Liquid::default_array());

            for e in fs::read_dir(working_dir).unwrap() {
                fn inner(e: Result<DirEntry, io::Error>, parent: Liquid) -> Result<(), io::Error> {
                    let e = e?;
                    let path = e.path();

                    match e.file_type()?.is_dir() {
                        true => create_folder(path, parent.as_object().io_result()?),
                        false => create_file(path, parent.as_object().io_result()?),
                    }
                }

                match inner(e, base.clone()) {
                    Ok(_) => (),
                    Err(e) => {
                        eprint!("\n\n{}\n\n", e);
                        continue;
                    }
                }
            }

            Some(())
        })
        .io_result()?;

        Ok(base)
    }

    #[deprecated]
    fn parse_nodes(plugins: &Plugins, node: &mut HashMap<String, Liquid>) {
        let folders = node
            .get(FOLDER_FIELDS_FOLDER)
            .expect("Object does not contain argument.")
            .as_array()
            .result(&())
            .unwrap();
        let mut files = node
            .get(FOLDER_FIELDS_FILES)
            .expect("Object does not contain argument.")
            .as_array()
            .result(&())
            .unwrap();

        for folder_liq in folders.iter() {
            let mut file = folder_liq.as_object().result(&()).unwrap();

            parse_nodes(plugins, &mut file)
        }

        for mut file_liq in files.iter_mut() {
            fn parse_file(plugins: &Plugins, object: &mut Liquid) -> Result<(), io::Error> {
                let file = object.as_object().result(&())?;
                let file = (&file).get_mut();

                let content = file.get(PAGE_CONTENT).unwrap().as_string().result(&())?;
                let path = file.get(PAGE_PATH).unwrap().as_string().result(&())?;

                let out_path_rel = &path[SITE.len() + 1..];
                let mut out_path = PathBuf::from(PAGE_OUT_FOLDER);
                out_path.push(out_path_rel);

                {
                    if let Some(out_str) = out_path.to_str() {
                        let out_str = out_str.replace(".md", ".html");
                        out_path = PathBuf::from(out_str)
                    } else {
                        eprintln!("Why")
                    }
                }

                let mut offset = 0;

                let layout = match file.get(PAGE_LAYOUT) {
                    Some(s) => s.as_string().result(&())?,
                    None => {
                        offset = file
                            .get(PAGE_FRONT_MATTER_OFFSET)
                            .expect("Created by application.")
                            .as_int()
                            .unwrap_or_default();

                        if offset != 0 {
                            offset += 3;
                        }

                        "paste_in"
                    } // TODO:(frac) get rid of str
                };

                fn get_layout(file_name: &str) -> Option<&String> {
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
                    "paste_in" => {
                        let state = LiquidState {
                            plugins,
                            file_path: path,
                            current_line: 0,
                            current_column: 0,
                        };

                        match parse(&state, &object, content, offset as _) {
                            Ok(parsed) => walk_write(out_path, &parsed),
                            Err(e) => Err(e.into()),
                        }
                    }
                    any => {
                        let state = LiquidState {
                            plugins,
                            file_path: &format!("layouts/{any}"),
                            current_line: 0,
                            current_column: 0,
                        };

                        match get_layout(any) {
                            Some(layout) => match parse(&state, &object, layout, offset as _) {
                                Ok(parsed) => walk_write(out_path, &parsed),
                                Err(e) => Err(e.into()),
                            },
                            None => Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("Layout `{any}` could not found."),
                            )),
                        }
                    }
                }
            }

            if let Err(e) = parse_file(plugins, &mut file_liq) {
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
            fn inner(e: Result<DirEntry, io::Error>) -> Result<(), io::Error> {
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

            match inner(e) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("\n\n{e}\n\n")
                }
            }
        }

        Ok(())
    }

    let working_dir = PathBuf::from("site");

    let parent = create_folder_starter(working_dir).unwrap();

    clear_dir("out").unwrap_or_else(|e| eprintln!("Could not clean the \"out\" folder. {e}"));

    let assets_path = PathBuf::from("assets");
    if !assets_path.exists() {
        fs::create_dir(&assets_path)
            .unwrap_or_else(|e| eprintln!("Could not create \"assets\" folder. {e}"));
    }

    generate_assets(assets_path)
        .unwrap_or_else(|e| eprintln!("Some error in \"assets\" folder. {e}"));
    parse_nodes(liquid_tags, &mut parent.as_object().unwrap());

    Ok(())
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

fn create_folder<P: AsRef<Path>>(path: P, parent_folder: MutRc<Object>) -> Result<(), io::Error> {
    let new_folder = Liquid::default_object();

    new_folder
        .with_object(|object| {
            object.insert(FOLDER_FIELDS_FOLDER.to_string(), Liquid::default_array());
            object.insert(FOLDER_FIELDS_FILES.to_string(), Liquid::default_array());

            object.insert(
                PAGE_PARENT.to_string(),
                LiquidInner::WeakObject(Rc::downgrade(&parent_folder.0)).into(),
            );

            Some(())
        })
        .unwrap();

    parent_folder
        .get(FOLDER_FIELDS_FOLDER)
        .expect("folders not found")
        .as_array()
        .result(&())?
        .push(new_folder.clone());

    for e in fs::read_dir(path)? {
        fn inner(e: Result<DirEntry, io::Error>, parent: MutRc<Object>) -> Result<(), io::Error> {
            let e = e?;
            let path = e.path();

            match e.file_type()?.is_dir() {
                true => create_folder(path, parent),
                false => create_file(path, parent),
            }
        }

        match inner(e, new_folder.as_object().unwrap()) {
            Ok(_) => (),
            Err(e) => {
                eprint!("{}", e);
                continue;
            }
        }
    }

    Ok(())
}

pub fn yaml_to_liquid(yaml: Yaml) -> Liquid {
    match yaml {
        Yaml::Real(s) => s.into(),
        Yaml::Integer(i) => (i as i64).into(),
        Yaml::String(s) => s.into(),
        Yaml::Boolean(b) => b.into(),
        Yaml::Array(a) => {
            let mut l = Vec::new();
            for y in a {
                l.push(yaml_to_liquid(y));
            }
            MutRc(Rc::new(UnsafeCell::new(l)), false).into()
        }
        Yaml::Hash(h) => {
            let mut hash = HashMap::new();

            for (k, v) in h {
                hash.insert(k.as_str().unwrap().to_string(), yaml_to_liquid(v));
            }

            MutRc(Rc::new(UnsafeCell::new(hash)), false).into()
        }
        Yaml::Alias(a) => (a as i64).into(),
        Yaml::Null => ().into(),
        Yaml::BadValue => ().into(),
    }
}

fn create_file(path: PathBuf, parent: MutRc<Object>) -> Result<(), io::Error> {
    let content: String = fs::read_to_string(&path)?;

    let path_str = match path.to_str() {
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

    let front_matter = {
        let mut yaml = match yaml_rust::YamlLoader::load_from_str(yaml_source) {
            Ok(y) => y,
            Err(e) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        };

        match yaml.pop() {
            Some(yaml) => yaml_to_liquid(yaml),
            None => Liquid::default_object(),
        }
    };

    front_matter
        .with_object(|front_matter| {
            front_matter.insert(PAGE_CONTENT.to_string(), content.clone().into());
            front_matter.insert(PAGE_PATH.to_string(), path_str.to_string().into());
            front_matter.insert(
                PAGE_FRONT_MATTER_OFFSET.to_string(),
                (front_matter_finish_index as i64).into(),
            );

            front_matter.insert(
                PAGE_PARENT.to_string(),
                LiquidInner::WeakObject(Rc::downgrade(&parent.0)).into(),
            );
            Some(())
        })
        .io_result()?;

    parent
        .get(FOLDER_FIELDS_FILES)
        .io_result()?
        .as_array()
        .io_result()?
        .push(front_matter);

    Ok(())
}

#[derive(Debug)]
pub struct StackInfo {
    info: String,
}

impl StackInfo {
    pub fn new_info(state: &LiquidState, to_parse: &str, found: regex::Match<'_>) -> Self {
        let (line, column) = get_line_and_column(to_parse, found.start());
        let line = line + state.current_line;
        Self {
            info: format!(
                "{path}:{line}:{column} {tag}",
                path = state.file_path,
                tag = found.as_str()
            ),
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub stack: Vec<StackInfo>,
    pub message: String,
}

impl<'a> ParseError {
    pub fn new(message: String) -> Self {
        ParseError {
            stack: Vec::new(),
            message,
        }
    }

    pub fn new_in_parse<'s>(
        message: String,
        state: &LiquidState,
        to_parse: &'s str,
        found: regex::Match<'s>,
    ) -> Self {
        Self {
            stack: vec![StackInfo::new_info(state, to_parse, found)],
            message,
        }
    }

    pub fn add_stack(
        mut self,
        state: &LiquidState,
        to_parse: &str,
        found: regex::Match<'_>,
    ) -> Self {
        self.stack.push(StackInfo::new_info(state, to_parse, found));
        self
    }
}

impl<'a> From<String> for ParseError {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl<'a> Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{message}", message = self.message)?;

        for s in &self.stack {
            writeln!(f, "\tat {}", s.info)?;
        }

        Ok(())
    }
}

impl std::error::Error for ParseError {}

impl Into<Error> for ParseError {
    fn into(self) -> Error {
        Error::new(io::ErrorKind::Other, self)
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

pub fn parse(
    state: &LiquidState,
    object: &Liquid,
    to_parse: &str,
    skip_offset: usize,
) -> Result<String, ParseError> {
    let mut parsed: Vec<String> = Vec::new();

    let mut internal_state = BlockState::Closed;
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
        let plugin_name = tag.split(' ').next().unwrap().trim();

        match internal_state {
            BlockState::Closed => {
                // ? SYNC:(non_matched_push)
                {
                    let mut non_matched_text = &to_parse[last_found_end..found.start()];
                    last_found_end = found.end();

                    if request_trim_start {
                        non_matched_text = non_matched_text.trim_start();
                    }

                    if trim_end {
                        non_matched_text = non_matched_text.trim_end();
                    }

                    parsed.push(non_matched_text.to_string());
                    request_trim_start = trim_start;
                }

                match &non_trimmed_tag[..2] {
                    "{{" => {
                        let mut tag_without_filter = tag;

                        let first_pipe = tag.find('|').unwrap_or_default();

                        let filters: Vec<&str>;
                        if first_pipe == 0 {
                            filters = Vec::new()
                        } else {
                            tag_without_filter = &tag[..first_pipe].trim();
                            filters = tag[first_pipe + 1..].split('|').map(|s| s.trim()).collect();
                        }

                        let mut target_object = match state.get_key(object, tag_without_filter) {
                            Ok(s) => s,
                            Err(e) => {
                                return Err(e.add_stack(state, to_parse, found));
                            }
                        };

                        for filter in filters {
                            if let Some(filter_proc) = state.plugins.filters.get(filter) {
                                match filter_proc(state, tag, &mut target_object) {
                                    Ok(success) => target_object = success,
                                    Err(e) => return Err(e.add_stack(state, to_parse, found)),
                                };
                            } else {
                                return Err(ParseError::new_in_parse(
                                    format!("Filter {filter} does not exists."),
                                    state,
                                    to_parse,
                                    found,
                                ));
                            }
                        }

                        // ? SYNC:(output_parser)
                        {
                            let output = target_object.as_string().result(state)?;

                            let mut output_not_trimmed = output.as_str();

                            if request_trim_start {
                                output_not_trimmed = output_not_trimmed.trim_start();
                            }

                            if trim_end {
                                output_not_trimmed = output_not_trimmed.trim_end();
                            }

                            parsed.push(output_not_trimmed.to_string());

                            request_trim_start = trim_start;
                        }
                    }
                    "{%" => {
                        if state.plugins.blocks.contains_key(plugin_name) {
                            internal_state = BlockState::Opened {
                                block_opener: found,
                                block_opener_tag: tag,
                                block_opener_plugin: plugin_name,
                                block_ender: "end".to_string() + plugin_name,
                            };
                            continue;
                        }

                        if let Some(tag_proc) = state.plugins.tags.get(plugin_name) {
                            let output = match tag_proc(state, object, tag) {
                                Ok(success) => success,
                                Err(e) => {
                                    return Err(e.add_stack(state, to_parse, found));
                                } // ! Handle error
                            };

                            // ? SYNC:(output_parser)
                            {
                                let output = output.as_string().result(state)?;

                                let mut output_not_trimmed = output.as_str();

                                if request_trim_start {
                                    output_not_trimmed = output_not_trimmed.trim_start();
                                }

                                if trim_end {
                                    output_not_trimmed = output_not_trimmed.trim_end();
                                }

                                parsed.push(output_not_trimmed.to_string());

                                request_trim_start = trim_start;
                            }

                            continue;
                        }

                        return Err(ParseError::new_in_parse(
                            format!("\"{plugin_name}\" does not exist in plugins."),
                            state,
                            to_parse,
                            found,
                        ));
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
                        internal_state = BlockState::Opened {
                            block_opener,
                            block_opener_tag,
                            block_opener_plugin,
                            block_ender,
                        };
                        continue;
                    }

                    internal_state = BlockState::Closed;
                    last_found_end = found.end();

                    let block_text = &to_parse[block_opener.end()..found.start()];

                    let plugin_proc = state
                        .plugins
                        .blocks
                        .get(block_opener_plugin)
                        .expect("Checked while opening block.");

                    let output = match plugin_proc(state, object, block_opener_tag, block_text) {
                        Ok(success) => success,
                        Err(e) => {
                            return Err(e.add_stack(state, to_parse, found));
                        }
                    };

                    // ? SYNC:(output_parser)
                    {
                        let output = output.as_string().result(state)?;

                        let mut output_not_trimmed = output.as_str();

                        if request_trim_start {
                            output_not_trimmed = output_not_trimmed.trim_start();
                        }

                        if trim_end {
                            output_not_trimmed = output_not_trimmed.trim_end();
                        }

                        parsed.push(output_not_trimmed.to_string());

                        request_trim_start = trim_start;
                    }
                } else {
                    if block_opener_plugin == plugin_name {
                        same_block_recursion += 1;
                    }
                    internal_state = BlockState::Opened {
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

pub fn get_object_path(state: &LiquidState, liquid: &Liquid) -> String {
    match state.get_key(liquid, "path") {
        Ok(s) => match s.as_string() {
            Some(s) => s.clone(),
            None => String::new(),
        },
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
