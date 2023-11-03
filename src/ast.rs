//! Common AST structs and traits

use std::{
    fmt::Debug,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::{
    cpath::{CPath, Filter},
    lexer::Literal,
    utils::ResolvePath,
};

pub trait FromLiteral: Eq + PartialEq + for<'a> From<Literal<'a>> + Clone + Default {}

impl<T: Eq + PartialEq + for<'a> From<Literal<'a>> + Clone + Default> FromLiteral for T {}

#[derive(Debug, Clone, Default)]
pub struct Config<S, T = String>
where
    S: Clone + Default,
    T: FromLiteral,
{
    pub path: PathBuf,
    pub root: Directive<S, T>,
}

impl<S, T> Config<S, T>
where
    Directive<S, T>: DirectiveTrait<S, T>,
    S: Clone + Default,
    T: FromLiteral,
{
    pub fn parse(path: PathBuf) -> anyhow::Result<Self> {
        let data = std::fs::read(&path)?;
        Ok(Config {
            path,
            root: Directive {
                children: Some(Directive::parse(&data)?),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    pub fn root_directives(&self) -> &[Directive<S, T>] {
        self.root
            .children
            .as_ref()
            .expect("root must have children")
    }

    pub fn resolve_include(
        &mut self,
        root_dir: Option<&Path>,
        res: Option<ResolvePath>,
    ) -> anyhow::Result<()> {
        self.root
            .resolve_include(root_dir.or(self.path.parent()).context("no root_dir")?, res)?;
        Ok(())
    }
}

pub trait DirectiveTrait<S, T = String>: Sized + AsMut<Directive<S, T>>
where
    Directive<S, T>: DirectiveTrait<S, T>,
    S: Clone + Default,
    T: FromLiteral,
{
    fn parse(input: &[u8]) -> anyhow::Result<Vec<Self>>;

    fn resolve_include(&mut self, dir: &Path, res: Option<ResolvePath>) -> anyhow::Result<()> {
        if let Some(childs) = self.as_mut().children.take() {
            let mut result = vec![];
            for c in childs {
                c.resolve_include_inner(dir, &mut result, res)?;
            }
            self.as_mut().children.replace(result);
        }
        Ok(())
    }

    fn resolve_include_inner(
        self,
        dir: &Path,
        out: &mut Vec<Self>,
        res: Option<ResolvePath>,
    ) -> anyhow::Result<()>;
}

#[derive(Clone, Default, Eq)]
pub struct Directive<S, T = String>
where
    S: Clone + Default,
    T: FromLiteral,
{
    pub name: T,
    pub args: Vec<T>,
    pub children: Option<Vec<Directive<S, T>>>,
    pub(crate) _scheme: PhantomData<S>,
    pub is_comment: bool,
}

impl<S: Debug, T: Debug> Debug for Directive<S, T>
where
    S: Clone + Default,
    T: FromLiteral,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Directive");
        ds.field("name", &self.name)
            .field("args", &self.args)
            .field("is_comment", &self.is_comment);
        if let Some(children) = self.children.as_ref() {
            ds.field("children", children);
        }
        ds.finish()
    }
}

impl<S, T> PartialEq for Directive<S, T>
where
    S: Clone + Default,
    T: FromLiteral,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.args == other.args && self.children == other.children
    }
}

impl<S, T> AsMut<Self> for Directive<S, T>
where
    S: Clone + Default,
    T: FromLiteral,
{
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<S, T> Directive<S, T>
where
    S: Clone + Default,
    T: FromLiteral + AsRef<str>,
{
    pub fn query(&self, path: &str) -> Vec<Self> {
        let mut result = vec![];
        if let Some(childs) = self.children.as_ref() {
            Self::inner_query(childs, path, &mut result);
        }
        result
    }

    fn inner_query(dirs: &[Self], path: &str, out: &mut Vec<Self>) {
        let mut pathitem = path;
        let mut rest = None;
        if let Some((i, r)) = path.split_once("/") {
            pathitem = i;
            rest.replace(r);
        }
        for d in dirs.iter() {
            if d.name.as_ref().eq_ignore_ascii_case(pathitem) {
                if let Some(path) = rest {
                    Self::inner_query(
                        d.children.as_ref().map(Vec::as_slice).unwrap_or(&[]),
                        path,
                        out,
                    );
                } else {
                    out.push(d.clone());
                }
            }
        }
    }

    pub fn cpath_query(&self, path: &CPath) -> Vec<Self> {
        let mut result = vec![];
        if let Some(childs) = self.children.as_ref() {
            Self::inner_cpath_query(childs, path, &mut result);
        }
        result
    }

    fn inner_cpath_query(dirs: &[Self], path: &CPath, out: &mut Vec<Self>) {
        let (item, rest, anylevel) = match path.peek() {
            Some(x) => x,
            _ => return,
        };

        for d in dirs.iter() {
            let childs = d.children.as_ref().map(Vec::as_slice).unwrap_or(&[]);
            if d.match_filter(&item.filter) {
                // leaf match
                if rest.is_empty() {
                    out.push(d.clone());
                } else {
                    Self::inner_cpath_query(childs, rest, out);
                }
            }
            if anylevel {
                Self::inner_cpath_query(childs, path, out);
            }
        }
    }

    fn match_filter(&self, filter: &Filter) -> bool {
        match filter {
            Filter::Eq(n) => self.name.as_ref().eq_ignore_ascii_case(n),
            Filter::Re(re) => re.is_match(self.name.as_ref()),
            Filter::Any => true,
            Filter::AnyLevel => false,
        }
    }
}
