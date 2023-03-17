//! Nom parser for nginx configuration

pub mod lexer;

use std::path::Path;

use crate::{
    ast::{Directive, DirectiveTrait},
    lexer::{line_column2, Literal},
};

use self::lexer::*;

use anyhow::Context;
use nom::{
    combinator::{fail, map},
    error::{ContextError, ParseError, VerboseError},
    multi::many0,
};

#[derive(Debug, Clone, Default)]
pub struct Nginx;

impl DirectiveTrait<Nginx> for Directive<Nginx> {
    fn parse(input: &[u8]) -> anyhow::Result<Vec<Self>> {
        let res = parse_block(input).map_err(|err| {
            err.map(|e| {
                let errs = e
                    .errors
                    .iter()
                    .map(|(i, code)| {
                        let ((l, c), pos) = line_column2(input, i).unwrap();
                        format!("0x{pos:x}({l}:{c}) err: {:?}", code)
                    })
                    .collect::<Vec<_>>();
                anyhow::anyhow!("{}", errs.join("\n"))
            })
        })?;
        Ok(res.1)
    }

    fn resolve_include_inner(mut self, dir: &Path, out: &mut Vec<Self>) -> anyhow::Result<()> {
        if self.name == "include" {
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

fn parse_literal(input: &[u8]) -> IResult<&[u8], Literal<'_>> {
    let (rest, tok) = tokenizer(input)?;
    match tok {
        Token::Literal(l) => Ok((rest, l)),
        Token::Eof | Token::BlockEnd => Ok((rest, Default::default())),
        _else => fail(input),
    }
}

fn parse_block(mut input: &[u8]) -> IResult<&[u8], Vec<Directive<Nginx>>> {
    let mut result = vec![];
    loop {
        let mut d = Directive::default();
        let (rest, tag) = tokenizer(input).map_err(|err| {
            err.map(|err| VerboseError::add_context(input, "unexpected item token", err))
        })?;

        d.name = match tag {
            Token::Literal(lit) => lit,
            Token::BlockEnd | Token::Eof => break,
            _ => return fail(input),
        }
        .into();

        let (rest, args) = map(many0(parse_literal), |v| {
            v.into_iter().map(Into::into).collect()
        })(rest)?;
        d.args = args;

        let (rest, tok) = tokenizer(rest)?;
        match tok {
            Token::Semicolon | Token::NewLine => {
                input = rest;
            }
            Token::Eof => break,
            Token::BlockStart => {
                let (rest, res) = parse_block(rest)?;
                d.children.replace(res);
                let (rest, tok) = tokenizer(rest)?;
                if tok != Token::BlockEnd {
                    return Err(nom::Err::Failure(VerboseError::add_context(
                        input,
                        "expected block end brace",
                        VerboseError::from_error_kind(input, nom::error::ErrorKind::Fail),
                    )));
                }
                input = rest;
            }
            _ => {
                fail::<_, (), _>(rest)?;
            }
        }

        result.push(d);
    }
    Ok((input, result))
}
