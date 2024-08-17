use std::{
    collections::{hash_map::Keys, HashMap},
    ffi::OsStr,
    fmt, io,
    path::PathBuf,
};

use crate::{
    helper::MutRef,
    liquid::Liquid,
    liquid::{LiquidInner, MutRc, OptionToResult},
    markdown::markdown,
    parser::{parse, LiquidResult, LiquidState, PAGE_CONTENT, PAGE_FRONT_MATTER_OFFSET},
};

pub type PfnLiquidFunction = fn(state: &LiquidState, object: &Liquid) -> LiquidResult;

pub type PfnLiquidTag = fn(state: &LiquidState, object: &Liquid, tag: &str) -> LiquidResult;
pub type PfnLiquidBlock =
    fn(state: &LiquidState, object: &Liquid, tag: &str, block: &str) -> LiquidResult;
pub type PfnLiquidFilter =
    fn(state: &LiquidState, filter: &str, property: &mut Liquid) -> LiquidResult;
pub type PfnLiquidAsset =
    fn(plugins: &Plugins, object: &Liquid, path: &OsStr) -> Result<(), io::Error>;

const LIQUID_TAGS: &[(&str, PfnLiquidTag)] = &[
    ("assign", pfn_assign),
    ("include", pfn_include),
    ("error", pfn_error),
];
const LIQUID_BLOCKS: &[(&str, PfnLiquidBlock)] = &[
    ("capture", pfn_capture),
    ("if", pfn_if_block),
    ("comment", pfn_comment),
];
const LIQUID_FILTERS: &[(&str, PfnLiquidFilter)] = &[("dbg", pfn_dbg)];
const LIQUID_FUNCTIONS: &[(&str, PfnLiquidFunction)] = &[
    ("content", pfn_content),
    ("len", pfn_len),
    ("dbg_fn", pfn_dbg_fn),
];
const LIQUID_ASSETS: &[(&str, PfnLiquidAsset)] = &[];

#[derive(Default)]
pub struct Plugins {
    pub tags: HashMap<String, PfnLiquidTag>,
    pub blocks: HashMap<String, PfnLiquidBlock>,
    pub filters: HashMap<String, PfnLiquidFilter>,
    pub functions: HashMap<String, PfnLiquidFunction>,
    pub assets: HashMap<String, PfnLiquidAsset>,
}

impl fmt::Debug for Plugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Active Plugins:
    Tags: {tags}
    Blocks: {blocks}
    Filters: {filters}
    Functions: {functions}
",
            tags = format_keys(self.tags.keys()),
            blocks = format_keys(self.blocks.keys()),
            filters = format_keys(self.filters.keys()),
            functions = format_keys(self.functions.keys()),
        )
    }
}

fn format_keys<T>(keys: Keys<String, T>) -> String {
    let mut out = Vec::new();
    for key in keys {
        out.push(key.as_str())
    }

    out.join(", ")
}

impl Plugins {
    pub fn new() -> Plugins {
        let mut this = Self::default();

        for (name, pfn) in LIQUID_TAGS {
            this.tags.insert(name.to_string(), *pfn);
        }

        for (name, pfn) in LIQUID_BLOCKS {
            this.blocks.insert(name.to_string(), *pfn);
        }

        for (name, pfn) in LIQUID_FILTERS {
            this.filters.insert(name.to_string(), *pfn);
        }

        for (name, pfn) in LIQUID_FUNCTIONS {
            this.functions.insert(name.to_string(), *pfn);
        }

        for (name, pfn) in LIQUID_ASSETS {
            this.assets.insert(name.to_string(), *pfn);
        }

        this
    }

    pub fn show(&self) {
        println!("{:?}", self);
    }
}

fn pfn_assign<'a>(state: &LiquidState, object: &Liquid, tag: &str) -> LiquidResult {
    let tags: Vec<&str> = tag.split(' ').map(|s| s.trim()).collect();
    let target = tags[1];
    let from = tags[3];

    let mut r = object.as_object().result(state)?;
    let i = state.get_key(object, from)?;

    r.insert(target.to_string(), i);

    Ok(().into())
}

fn pfn_include<'a>(state: &LiquidState, object: &Liquid, tag: &str) -> LiquidResult {
    let a: Vec<&str> = tag.split(" ").collect();
    let include_file = a.last().unwrap().trim();

    let mut new_state = LiquidState {
        file_path: "",
        current_line: 0,
        ..*state
    };

    let mut path = PathBuf::from("includes");

    let s = if include_file.starts_with("./") {
        new_state.file_path = include_file;
        match std::fs::read_to_string(include_file) {
            Ok(s) => s,
            Err(e) => return Err(format!("{include_file} is not exists! {e}").into()),
        }
    } else {
        if !path.exists() {
            match std::fs::create_dir(&path) {
                Ok(_) => (),
                Err(e) => return Err(format!("{e}").into()),
            }

            return Err(format!("{include_file} is not exists!").into());
        }

        path.push(include_file);

        new_state.file_path = path.to_str().unwrap_or(include_file);

        match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => return Err(format!("{include_file} is not exists! {e}").into()),
        }
    };

    Ok(parse(&new_state, object, &s, 0)?.into())
}

fn pfn_error(_: &LiquidState, _: &Liquid, tag: &str) -> LiquidResult {
    Err(format!("Error Tag {}", tag).into())
}

// {% capture a %}
fn pfn_capture(
    state: &LiquidState,
    object: &Liquid,
    tag: &str,
    block_contents: &str,
) -> LiquidResult {
    let tag_s: Vec<&str> = tag.trim().split(" ").map(|s| s.trim()).collect();
    let tag_name = match tag_s.get(1) {
        Some(some) => some,
        None => return Err(format!("Invalid arguments {:?}", tag).into()),
    };

    let parsed_contents = parse(state, object, block_contents, 0)?;

    let binding = object.as_object().unwrap();
    let f = binding.get_mut();
    f.insert(tag_name.to_string(), parsed_contents.into());

    Ok(().into())
}

fn pfn_comment(_: &LiquidState, _: &Liquid, _: &str, block_contents: &str) -> LiquidResult {
    Ok(block_contents.into())
}

#[deprecated]
fn pfn_if_block(
    state: &LiquidState,
    object: &Liquid,
    tag: &str,
    block_contents: &str,
) -> LiquidResult {
    let tokens: Vec<&str> = tag.trim().split(' ').map(|s| s.trim()).collect();

    if tokens.len() != 2 {
        return Err(format!("TODO! {}", tokens.len()).into());
    }

    let condition = state.get_key(object, tokens[1])?;

    if !condition.is() {
        return Ok(().into());
    }

    Ok(parse(state, object, block_contents, 0)?.into())
}

fn pfn_dbg(s: &LiquidState, _full_tag: &str, prop: &mut Liquid) -> LiquidResult {
    pfn_dbg_fn(s, &prop)
}

fn pfn_len(_state: &LiquidState, liq: &Liquid) -> LiquidResult {
    if let Some(len) = liq.len() {
        Ok((len as i64).into())
    } else {
        Ok(().into())
    }
}

fn pfn_content(state: &LiquidState, page: &Liquid) -> LiquidResult {
    let binding = state.get_key(page, PAGE_CONTENT)?;
    let raw_content = binding.as_string().result(state)?;
    let mut offset = state
        .get_key(page, PAGE_FRONT_MATTER_OFFSET)?
        .as_int()
        .result(state)?;
    let binding = state.get_key(page, "path")?;
    let path = binding.as_string().result(state)?;

    if offset != 0 {
        offset += 3;
    }

    let object = page.clone();

    let new_state = LiquidState {
        plugins: state.plugins,
        file_path: path,
        ..*state
    };
    Ok(markdown(&parse(&new_state, &object, &raw_content, offset as usize)?).into())
}

fn pfn_dbg_fn(_state: &LiquidState, object: &Liquid) -> LiquidResult {
    let object2: Liquid = match object.inner.mut_ref() {
        LiquidInner::WeakObject(v) => {
            if let Some(v) = v.upgrade() {
                MutRc(v, false).into()
            } else {
                object.clone()
            }
        }
        LiquidInner::WeakArray(v) => {
            if let Some(v) = v.upgrade() {
                MutRc(v, false).into()
            } else {
                object.clone()
            }
        }
        o => o.clone().into(),
    };

    Ok((format!("{:#?}", object2)).into())
}
