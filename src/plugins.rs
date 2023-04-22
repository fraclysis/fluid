use std::{
    collections::{hash_map::Keys, HashMap},
    ffi::OsStr,
    fmt, io,
    path::PathBuf,
    rc::Rc,
};

use crate::{
    cast_mut, get_line_and_column, markdown::markdown, parse, Liquid, PAGE_CONTENT,
    PAGE_FRONT_MATTER_OFFSET,
};

static mut PLUGINS: Option<Plugins> = None;

pub type LiquidFunction = fn(&Liquid) -> Result<Liquid, String>;

pub type LiquidTag = fn(full_tag: &str, object: &mut Liquid) -> Result<Liquid, String>;
pub type LiquidBlock =
    fn(full_tag: &str, block: &str, object: &mut Liquid) -> Result<Liquid, String>;
pub type LiquidFilter =
    fn(full_tag: &str, property: &mut Liquid, object: &mut Liquid) -> Result<Liquid, String>;

pub type AssetFunction = fn(path: &OsStr) -> Result<(), io::Error>;

#[derive(Default)]
pub struct Plugins {
    pub tags: HashMap<String, LiquidTag>,
    pub blocks: HashMap<String, LiquidBlock>,
    pub filters: HashMap<String, LiquidFilter>,
    pub functions: HashMap<String, LiquidFunction>,
    pub assets: HashMap<String, AssetFunction>,
}

impl fmt::Debug for Plugins {
    // fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //     f.debug_struct("LiquidTags")
    //         .field("tags", &self.tags.keys())
    //         .field("blocks", &self.blocks.keys())
    //         .field("filters", &self.filters.keys())
    //         .field("functions", &self.functions.keys())
    //         .finish()
    // }

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
    pub fn init() {
        unsafe { PLUGINS = Some(Plugins::default()) }

        let this = Plugins::get();

        init_tags(&mut this.tags);
        init_blocks(&mut this.blocks);
        init_filters(&mut this.filters);
        init_funtions(&mut this.functions);
    }

    pub fn terminate() {
        unsafe { PLUGINS = None };
    }

    pub fn get() -> &'static mut Plugins {
        unsafe {
            match &mut PLUGINS {
                Some(p) => p,
                None => unreachable!(),
            }
        }
    }

    pub fn show() {
        println!("{:?}", Plugins::get());
    }
}

pub fn init_tags(oparator: &mut HashMap<String, LiquidTag>) {
    oparator.insert("include".to_string(), include);
    oparator.insert("err".to_string(), err);
    oparator.insert("assign".to_string(), assign);
}

fn assign(tag: &str, object: &mut Liquid) -> Result<Liquid, String> {
    let tags: Vec<&str> = tag.split(' ').map(|s| s.trim()).collect();
    let target = tags[1];
    let from = tags[3];

    let r = object.as_hash();
    let o = cast_mut(r.as_ref());
    o.insert(target.to_string(), object.get_prop(from)?);

    Ok(Liquid::Nil)
}

fn include(tag: &str, object: &mut Liquid) -> Result<Liquid, String> {
    let mut path = PathBuf::from("includes");
    let a: Vec<&str> = tag.split(" ").collect();
    let include_file = a.last().unwrap().trim();

    if !path.exists() {
        match std::fs::create_dir(&path) {
            Ok(_) => (),
            Err(e) => return Err(format!("{e}")),
        }

        return Err(format!("{include_file} is not exists!"));
    }

    path.push(include_file);

    let s = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return Err(format!("{include_file} is not exists! {e}")),
    };

    let liq = match parse(&s, object, 0) {
        Ok(liq) => liq,
        Err(e) => return Err(format!("{e}")),
    };
    Ok(Liquid::String(liq))
}

fn err(_tag: &str, _object: &mut Liquid) -> Result<Liquid, String> {
    Ok(Liquid::Nil)
}

pub fn init_blocks(oparator: &mut HashMap<String, LiquidBlock>) {
    oparator.insert("capture".to_string(), capture);
    oparator.insert("for".to_string(), for_loop);
    oparator.insert("if".to_string(), if_block);
}

// {% capture a = b %}
fn capture(tag: &str, block_contents: &str, object: &mut Liquid) -> Result<Liquid, String> {
    let tag_s: Vec<&str> = tag.trim().split(" ").map(|s| s.trim()).collect();
    let tag_name = tag_s.get(1).unwrap();

    let parsed_contents = parse(block_contents, object, 0).unwrap();

    let binding = object.as_hash();
    let f = cast_mut(binding.as_ref());
    f.insert(tag_name.to_string(), Liquid::String(parsed_contents));

    Ok(Liquid::Nil)
}

fn for_loop(tag: &str, block_contents: &str, object: &mut Liquid) -> Result<Liquid, String> {
    let tokens: Vec<&str> = tag.split(" ").collect();
    let iter_name = tokens[1].to_string();
    let target_name = tokens[3];

    let target = object.get_prop(target_name)?;
    let target_len = target.get_prop("len")?.as_int();

    let a = target.as_vec();
    let tar = cast_mut(a.as_ref());

    let mut con = Vec::new();

    for i in 0..target_len {
        // ! iterating over a container that strores self causes memory leak
        let mut iter_val = tar.get_mut(i as usize).unwrap().clone();

        match iter_val {
            Liquid::Object(o) => iter_val = Liquid::WeakObject(Rc::downgrade(&o)),
            Liquid::Array(o) => iter_val = Liquid::WeakArray(Rc::downgrade(&o)),
            _ => (),
        }

        {
            let b = object.as_hash();
            let o = cast_mut(b.as_ref());
            let c = o;
            c.insert(iter_name.clone(), iter_val);
        }

        con.push(match parse(block_contents, object, 0) {
            Ok(o) => o,
            Err(e) => return Err(format!("{e}")),
        });
    }

    Ok(Liquid::String(con.join("")))
}

fn if_block(tag: &str, block_contents: &str, object: &mut Liquid) -> Result<Liquid, String> {
    let tokens = helper::get_tokens(tag);

    match tokens.len() {
        2 => {
            let token_to_check_if_exitst = &tokens[1];
            let token = match object.get_prop(&token_to_check_if_exitst) {
                Ok(o) => o,
                Err(e) => return Err(e),
            };
            match token {
                Liquid::Nil => return Ok(Liquid::String("".to_string())),
                _ => return Ok(Liquid::String(parse(block_contents, object, 0).unwrap())),
            }
        }
        _ => return Err(format!("Invalid tokens {:?}.", &tokens)),
    };
}

mod helper {
    pub fn get_tokens(text: &str) -> Vec<&str> {
        let mut tokens = Vec::new();

        for token in text.split(" ") {
            if token == "" {
                continue;
            }

            tokens.push(token);
        }

        tokens
    }
}

pub fn init_filters(oparator: &mut HashMap<String, LiquidFilter>) {
    oparator.insert("dbg".to_string(), dbg);
}

fn dbg(_full_tag: &str, prop: &mut Liquid, _object: &mut Liquid) -> Result<Liquid, String> {
    match prop {
        Liquid::WeakObject(e) => match e.upgrade() {
            Some(prop) => Ok(Liquid::String(format!("{:#?}", prop))),
            None => Ok(Liquid::String(format!("(Weak<Null<Object>>)"))),
        },
        Liquid::WeakArray(e) => match e.upgrade() {
            Some(prop) => Ok(Liquid::String(format!("{:#?}", prop))),
            None => Ok(Liquid::String(format!("(Weak<Null<Array>>)"))),
        },
        prop => Ok(Liquid::String(format!("{:#?}", prop))),
    }
}

pub fn init_funtions(oparator: &mut HashMap<String, LiquidFunction>) {
    oparator.insert("content".to_string(), content);
    oparator.insert("len".to_string(), len);
}

fn len(liq: &Liquid) -> Result<Liquid, String> {
    let res = match liq {
        Liquid::String(s) => Liquid::Int(s.len() as i32),
        Liquid::Int(_) => todo!(),
        Liquid::Object(a) => Liquid::Int(a.len() as i32),
        Liquid::Array(a) => Liquid::Int(a.len() as i32),
        Liquid::Bool(_) => todo!(),
        Liquid::WeakObject(a) => match a.upgrade() {
            Some(a) => Liquid::Int(a.len() as i32),
            None => Liquid::Int(0),
        },
        Liquid::WeakArray(a) => match a.upgrade() {
            Some(a) => Liquid::Int(a.len() as i32),
            None => Liquid::Int(0),
        },
        Liquid::Nil => Liquid::Int(0),
    };

    Ok(res)
}

pub fn content(page: &Liquid) -> Result<Liquid, String> {
    let raw_content = page.get_prop(PAGE_CONTENT)?.as_string();
    let mut offset = page.get_prop(PAGE_FRONT_MATTER_OFFSET)?.as_int();

    if offset != 0 {
        offset += 3;
    }

    let mut object = page.clone();

    match parse(&raw_content, &mut object, offset as usize) {
        Ok(o) => Ok(Liquid::String(markdown(&o))),
        Err(e) => {
            let (line, column) = get_line_and_column(e.content, e.tag_start);
            let tag = &e.content[e.tag_start..e.tag_end];
            let message = &e.error_message;
            let path = &e.error_path;
            Err(format!(
                "Failed to parse {tag} in \"{path}:{line}:{column}\" with message {message}"
            ))
        }
    }
}
