use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

pub type ResolvePath<'a> = &'a dyn Fn(&Path) -> anyhow::Result<PathBuf>;

pub trait PathResolver {
    fn resolve<'a>(&self, path: &'a Path) -> anyhow::Result<Cow<'a, Path>>;
}

impl PathResolver for Option<ResolvePath<'_>> {
    fn resolve<'a>(&self, path: &'a Path) -> anyhow::Result<Cow<'a, Path>> {
        Ok(if let Some(resolve) = self {
            Cow::Owned(resolve(path)?)
        } else {
            Cow::Borrowed(path)
        })
    }
}

pub fn replace_slice<T>(source: &[T], from: &[T], to: &[T]) -> Vec<T>
where
    T: Clone + PartialEq,
{
    let mut result = source.to_vec();
    let from_len = from.len();
    let to_len = to.len();

    let mut i = 0;
    while i + from_len <= result.len() {
        if result[i..].starts_with(from) {
            result.splice(i..i + from_len, to.iter().cloned());
            i += to_len;
        } else {
            i += 1;
        }
    }

    result
}
