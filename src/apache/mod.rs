//! Nom parser for apache configuration

pub mod lexer;

use std::path::Path;

use crate::{
    ast::{Directive, DirectiveTrait},
    lexer::line_column2,
    utils::*,
};

use self::lexer::*;

use anyhow::Context;
use nom::{
    character::complete::space0,
    combinator::{fail, map, map_opt, opt, verify},
    sequence::tuple,
    IResult,
};

#[derive(Debug, Clone, Default)]
pub struct Apache;

impl DirectiveTrait<Apache> for Directive<Apache> {
    fn parse(input: &[u8]) -> anyhow::Result<Vec<Self>> {
        let res = parse_block(input).map_err(|err| {
            err.map(|e| {
                let ((l, c), pos) = line_column2(input, e.input).unwrap();
                anyhow::anyhow!("{pos}({l}:{c}) err: {:?}", e.code)
            })
        })?;
        Ok(res.1)
    }

    fn resolve_include_inner(
        mut self,
        dir: &Path,
        out: &mut Vec<Self>,
        res: Option<ResolvePath>,
    ) -> anyhow::Result<()> {
        let optional = self.name.eq_ignore_ascii_case("IncludeOptional");
        if self.name.eq_ignore_ascii_case("include") || optional {
            let path = Path::new(
                self.args
                    .get(0)
                    .context("include directive expect one arg")?
                    .as_str(),
            );
            for path in glob::glob(
                &res.resolve(&if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    dir.join(path)
                })?
                .to_string_lossy(),
            )?
            .flatten()
            {
                let path = res.resolve(&path)?;
                if optional && !path.exists() {
                    continue;
                }
                let data = std::fs::read(&path)?;
                for c in Self::parse(&data).with_context(|| format!("parse {path:?}"))? {
                    c.resolve_include_inner(dir, out, res)?;
                }
            }
        } else {
            self.resolve_include(dir, res)?;
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
            let (mut rest, name) = map_opt(tokenizer, |tok| tok.ident())(input)?;
            let mut special = vec![];
            if name.eq_ignore_ascii_case("IfVersion") {
                let op;
                (rest, _) = opt(space0)(rest)?;
                (rest, op) = opt(lexer::operator_str)(rest)?;
                op.map(|op| special.push(op.to_string()));
            }
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
                move |(args, _, children, _)| {
                    special.extend(args);
                    Directive {
                        name: name.into(),
                        args: std::mem::take(&mut special),
                        children: Some(children),
                        ..Default::default()
                    }
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
