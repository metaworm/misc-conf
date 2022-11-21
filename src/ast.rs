use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::lexer::Literal;

pub trait Lit = Eq + PartialEq + for<'a> From<Literal<'a>> + Clone + Default;

#[derive(Debug, Clone, Default)]
pub struct Config<S, T = String>
where
    S: Clone + Default,
    T: Lit,
{
    pub path: PathBuf,
    pub root: Directive<S, T>,
    pub(crate) _scheme: PhantomData<S>,
}

impl<S, T> Config<S, T>
where
    Directive<S, T>: DirectiveTrait<S, T>,
    S: Clone + Default,
    T: Lit,
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

    pub fn resolve_include(&mut self, root_dir: Option<&Path>) -> anyhow::Result<()> {
        self.root
            .resolve_include(root_dir.or(self.path.parent()).context("no root_dir")?)?;
        Ok(())
    }
}

pub trait DirectiveTrait<S, T = String>: Sized + AsMut<Directive<S, T>>
where
    Directive<S, T>: DirectiveTrait<S, T>,
    S: Clone + Default,
    T: Lit,
{
    fn parse(input: &[u8]) -> anyhow::Result<Vec<Self>>;

    fn resolve_include(&mut self, dir: &Path) -> anyhow::Result<()> {
        if let Some(childs) = self.as_mut().children.take() {
            let mut result = vec![];
            for c in childs {
                c.resolve_include_inner(dir, &mut result)?;
            }
            self.as_mut().children.replace(result);
        }
        Ok(())
    }

    fn resolve_include_inner(self, dir: &Path, out: &mut Vec<Self>) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Default, Eq)]
pub struct Directive<S, T = String>
where
    S: Clone + Default,
    T: Lit,
{
    pub name: T,
    pub args: Vec<T>,
    pub children: Option<Vec<Directive<S, T>>>,
    pub(crate) _scheme: PhantomData<S>,
}

impl<S, T> PartialEq for Directive<S, T>
where
    S: Clone + Default,
    T: Lit,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.args == other.args && self.children == other.children
    }
}

impl<S, T> AsMut<Self> for Directive<S, T>
where
    S: Clone + Default,
    T: Lit,
{
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<S, T> Directive<S, T>
where
    S: Clone + Default,
    T: Lit,
{
    pub fn query(&self, path: &str) -> Vec<Self>
    where
        T: AsRef<str>,
    {
        let mut result = vec![];
        if let Some(childs) = self.children.as_ref() {
            Self::inner_query(childs, path, &mut result);
        }
        result
    }

    fn inner_query(dirs: &[Self], path: &str, out: &mut Vec<Self>)
    where
        T: AsRef<str>,
    {
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
}
