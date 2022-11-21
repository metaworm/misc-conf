pub mod lexer;

use std::path::Path;

use crate::ast::{Directive, DirectiveTrait};

use self::lexer::*;

use anyhow::Context;
use nom::{
    combinator::{fail, map, map_opt, verify},
    sequence::tuple,
    IResult,
};

#[derive(Debug, Clone, Default)]
pub struct Apache;

impl DirectiveTrait<Apache> for Directive<Apache> {
    fn parse(input: &[u8]) -> anyhow::Result<Vec<Self>> {
        let res = parse_block(input).map_err(|err| {
            err.map(|e| {
                let pos = unsafe { e.input.as_ptr().sub_ptr(input.as_ptr()) };
                let (l, c) = line_column(input, pos);
                anyhow::anyhow!("{pos}({l}:{c}) err: {:?}", e.code)
            })
        })?;
        Ok(res.1)
    }

    fn resolve_include_inner(mut self, dir: &Path, out: &mut Vec<Self>) -> anyhow::Result<()> {
        let optional = self.name.eq_ignore_ascii_case("IncludeOptional");
        if self.name.eq_ignore_ascii_case("include") || optional {
            let path = Path::new(
                self.args
                    .get(0)
                    .context("include directive expect one arg")?
                    .as_str(),
            );
            for path in glob::glob(
                &if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    dir.join(path)
                }
                .to_string_lossy(),
            )?
            .flatten()
            {
                if optional && !path.exists() {
                    continue;
                }
                let data = std::fs::read(&path)?;
                for c in Self::parse(&data).with_context(|| format!("parse {path:?}"))? {
                    c.resolve_include_inner(dir, out)?;
                }
            }
        } else {
            self.resolve_include(dir)?;
            out.push(self);
        }
        Ok(())
    }
}

fn parse_block(mut input: &[u8]) -> IResult<&[u8], Vec<Directive<Apache>>> {
    let mut result = vec![];
    loop {
        let (rest, tok) = tokenizer(input)?;
        match tok {
            Token::NewLine => {
                input = rest;
                continue;
            }
            Token::CloseTag | Token::Eof => break,
            _ => {}
        }
        let res = parse_one(rest, tok)?;
        result.push(res.1);
        input = res.0;
    }
    Ok((input, result))
}

fn parse_one<'a>(input: &'a [u8], tok: Token<'a>) -> IResult<&'a [u8], Directive<Apache>> {
    match tok {
        Token::OpenTag => {
            let (rest, name) = map_opt(tokenizer, |tok| tok.ident())(input)?;
            map(
                tuple((
                    parse_args::<false>,
                    verify(tokenizer, |tok| tok == &Token::EndTag),
                    parse_block,
                    verify(tuple((tokenizer, tokenizer, tokenizer)), |&(b, tag, e)| {
                        b == Token::CloseTag
                            && tag
                                .raw_string()
                                .unwrap_or_default()
                                .eq_ignore_ascii_case(name)
                            && e == Token::EndTag
                    }),
                )),
                |(args, _, children, _)| Directive {
                    name: name.into(),
                    args,
                    children: Some(children),
                    ..Default::default()
                },
            )(rest)
        }
        Token::Literal(l) => map(parse_args::<true>, |args| Directive {
            name: l.raw.into(),
            args,
            children: None,
            ..Default::default()
        })(input),
        _ => fail(input),
    }
}

fn parse_args<const NL: bool>(mut input: &[u8]) -> IResult<&[u8], Vec<String>> {
    let mut result = vec![];
    loop {
        let (rest, tok) = inner_tokenizer::<NL>(input)?;
        if let Some(l) = tok.literal() {
            result.push(l.into());
        } else {
            break;
        }
        input = rest;
    }
    // println!("[args] {result:?}");
    Ok((input, result))
}

fn line_column(data: &[u8], pos: usize) -> (usize, usize) {
    let mut ln = 1;
    for line in data.split(|&b| b == b'\n') {
        let lp = unsafe { line.as_ptr().sub_ptr(data.as_ptr()) };
        if (lp..=lp + line.len()).contains(&pos) {
            return (ln, pos - lp);
        }
        ln += 1;
    }
    (ln, 0)
}
