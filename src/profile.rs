mod de;

use crate::pattern::PatternSetBuilder;

pub struct Profile {
    root: ProfileNode,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ProfileAttr {
    pub from: Option<String>,
    pub link: Option<AttrLink>,
    pub tmpl: Option<bool>,
    pub ignore: Option<PatternSetBuilder>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum AttrLink {
    #[default]
    False,
    True,
    Recursive,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ProfileNode {
    attr: ProfileAttr,
    children: Vec<(String, ProfileNode)>,
}
