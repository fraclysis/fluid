pub struct Folder;
impl Folder {
    pub const FILES: &'static str = "files";
    pub const FOLDERS: &'static str = "folders";
}

pub struct Page;
impl Page {
    pub const CONTENT: &'static str = "content";
    pub const PARENT: &'static str = "parent";
    pub const OUTPUT: &'static str = "output";
    pub const LAYOUT: &'static str = "layout";
    pub const FRONT_MATTER_OFFSET: &'static str = "_front_matter_offset";
}
