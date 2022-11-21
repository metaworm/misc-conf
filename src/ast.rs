use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

use anyhow::Context;

#[derive(Debug, Clone, Default)]
pub struct Config<S: Clone> {
    pub path: PathBuf,
    pub root: Directive<S>,
    pub(crate) _scheme: PhantomData<S>,
}

impl<S: Clone> Config<S> {
    pub fn parse(path: PathBuf) -> anyhow::Result<Self>
    where
        Directive<S>: DirectiveTrait<S>,
        S: Default,
    {
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

    pub fn root_directives(&self) -> &[Directive<S>] {
        self.root
            .children
            .as_ref()
            .expect("root must have children")
    }

    pub fn resolve_include(&mut self, root_dir: Option<&Path>) -> anyhow::Result<()>
    where
        Directive<S>: DirectiveTrait<S>,
    {
        self.root
            .resolve_include(root_dir.or(self.path.parent()).context("no root_dir")?)?;
        Ok(())
    }
}

pub trait DirectiveTrait<S: Clone>: Sized + AsMut<Directive<S>>
where
    Directive<S>: DirectiveTrait<S>,
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
pub struct Directive<S: Clone> {
    pub name: String,
    pub args: Vec<String>,
    pub children: Option<Vec<Directive<S>>>,
    pub(crate) _scheme: PhantomData<S>,
}

impl<S: Clone> PartialEq for Directive<S> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.args == other.args && self.children == other.children
    }
}

impl<S: Clone> AsMut<Self> for Directive<S> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<S: Clone> Directive<S> {
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
            if d.name.eq_ignore_ascii_case(pathitem) {
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
