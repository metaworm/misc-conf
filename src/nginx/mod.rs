//! Nom parser for nginx configuration

pub mod lexer;

use std::path::Path;

use crate::{
    ast::{Directive, DirectiveTrait},
    lexer::{line_column2, Literal},
    utils::*,
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
        let res = parse_block(input)
            .map(|(i, r)| {
                let r = Directive {
                    children: Some(r),
                    ..Directive::default()
                };
                (i, vec![r])
            })
            .map_err(|err| {
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

    fn resolve_include_inner(
        mut self,
        dir: &Path,
        out: &mut Vec<Self>,
        res: Option<ResolvePath>,
    ) -> anyhow::Result<()> {
        if self.name == "include" {
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
                let data = std::fs::read(res.resolve(&path)?)?;
                let mut sub = Self::parse(&data)
                    .with_context(|| format!("parse {path:?}"))?;
                let sub = sub
                    .iter_mut()
                    .flat_map(|x|x.children.as_mut().cloned())
                    .flatten()
                    .collect::<Vec<_>>();
                for c in sub {
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
    let mut before_is_literal = None;
    loop {
        let mut d = Directive::default();
        let (rest, tag) = tokenizer(input).map_err(|err| {
            err.map(|err| VerboseError::add_context(input, "unexpected item token", err))
        })?;

        let lit = match tag {
            Token::Literal(lit) => lit,
            Token::BlockEnd | Token::Eof => break,
            Token::Comment(c) => {
                d.is_comment = true;
                d.name = c.raw.to_string();
                if before_is_literal != Some(false) {
                    before_is_literal = Some(false);
                    d.newline = true;
                }
                result.push(d);
                input = rest;
                continue;
            }
            _ => return fail(input),
        };

        if before_is_literal != Some(true) {
            d.newline = true;
        }
        before_is_literal = Some(true);
        d.name = lit.clone().into();
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
            Token::BlockStart if lit.raw.ends_with("_by_lua_block") => {
                use luaparse::token::*;

                let mut pairs = 1usize;
                let cow = String::from_utf8_lossy(rest);
                let mut lexer = luaparse::Lexer::new(luaparse::InputCursor::new(&cow));
                while let Some(Ok(tok)) = lexer.next() {
                    match tok.value {
                        TokenValue::Symbol(Symbol::CurlyBracketLeft) => pairs += 1,
                        TokenValue::Symbol(Symbol::CurlyBracketRight) => pairs -= 1,
                        _ => {}
                    }
                    if pairs == 0 {
                        break;
                    }
                }

                input = &rest[lexer.cursor().pos().byte..];
            }
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
                d.newline = false;
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
