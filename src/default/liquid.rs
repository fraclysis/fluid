pub struct Liquid {}

pub struct Object(HashMap<String, Liquid>);
pub struct Array(Vec<Liquid>);

use std::collections::HashMap;
