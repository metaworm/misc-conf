use std::{ops::Deref, path::Path};

use crate::lexer::Literal;
use nom::{
    branch::alt,
    bytes::complete::{escaped, tag},
    character::{
        complete::{char as cchar, multispace0, none_of},
        streaming::anychar,
    },
    combinator::{eof, fail, map, map_opt, map_res, opt, value},
    error::{context, VerboseError},
    number::complete::be_u8,
    sequence::{delimited, tuple},
    Parser,
};
use regex::Regex;

pub type IResult<I, O> = nom::IResult<I, O, VerboseError<I>>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Token<'a> {
    Slash,
    DoubleSlash,
    LeftBracket,
    RightBracket,
    Eof,
    Operator(Op),
    Literal(Literal<'a>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Op {
    Equal,
    Match,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CPathBuf(pub Vec<Item>);

impl<'a> Deref for CPathBuf {
    type Target = CPath;

    fn deref(&self) -> &Self::Target {
        CPath::new(&self.0)
    }
}

impl CPathBuf {
    pub fn parse(path: &str) -> anyhow::Result<Self> {
        let res = parse_cpath(path.as_ref()).map_err(|err| {
            err.map(|e| {
                let errs = e
                    .errors
                    .iter()
                    .map(|(input, code)| {
                        let pos = unsafe { input.as_ptr().sub_ptr(input.as_ptr()) };
                        let (l, c) = line_column(input, pos);
                        format!("0x{pos:x}({l}:{c}) err: {:?}", code)
                    })
                    .collect::<Vec<_>>();
                anyhow::anyhow!("{}", errs.join("\n"))
            })
        })?;
        Ok(res.1)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct CPath(pub [Item]);

impl Deref for CPath {
    type Target = [Item];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CPath {
    pub fn new<'a>(items: &'a [Item]) -> &'a Self {
        unsafe { &*(items as *const _ as *const Self) }
    }

    pub fn peek<'a>(&'a self) -> Option<(&'a Item, &'a Self, bool)> {
        let (mut first, mut rest) = self.0.split_first()?;
        let mut anylevel = false;
        while first.filter.any_level() {
            anylevel = true;
            let (a, b) = rest.split_first()?;
            first = a;
            rest = b;
        }
        Some((first, Self::new(rest), anylevel))
    }
}

#[derive(Debug)]
pub struct Item {
    pub filter: Box<Filter>,
    pub cond: Option<Box<Cond>>,
}

#[derive(Debug)]
pub enum Filter {
    Eq(String),
    Re(Regex),
    Any,
    AnyLevel,
}

impl Filter {
    pub fn any_level(&self) -> bool {
        matches!(self, Self::AnyLevel)
    }
}

#[derive(Debug)]
pub enum Cond {
    Exists(Regex),
    ChildExists(Item),
    Equal { name: String, value: String },
    Match { name: String, regex: Regex },
}

fn literal(input: &[u8]) -> IResult<&[u8], Token> {
    let (_, mut first) = be_u8(input)?;

    let (input, raw) = match first {
        b'"' => map_res(
            // for empty string
            map(
                delimited(
                    cchar(first as _),
                    opt(escaped(none_of(r#"\""#), '\\', anychar)),
                    cchar(first as _),
                ),
                Option::unwrap_or_default,
            ),
            std::str::from_utf8,
        )(input),
        b'\'' => map_res(
            // for empty string
            map(
                delimited(
                    cchar(first as _),
                    opt(escaped(none_of(r#"\'"#), '\\', anychar)),
                    cchar(first as _),
                ),
                Option::unwrap_or_default,
            ),
            std::str::from_utf8,
        )(input),
        _ => {
            first = 0;
            map_res(
                escaped(none_of(" \t\r\n'\"\\[]/=~"), '\\', anychar),
                std::str::from_utf8,
            )(input)
        }
    }?;
    Ok((input, Token::Literal(Literal { raw, quote: first })))
}

pub fn token(input: &[u8]) -> IResult<&[u8], Token> {
    use Token::*;

    map(
        tuple((
            multispace0,
            alt((
                eof.map(|_| Eof),
                value(DoubleSlash, tag(b"//")),
                value(Slash, tag(b"/")),
                value(LeftBracket, tag(b"[")),
                value(RightBracket, tag(b"]")),
                value(Operator(Op::Equal), tag(b"=")),
                value(Operator(Op::Match), tag(b"~")),
                literal,
            )),
        )),
        |x| x.1,
    )(input)
}

fn expect_map<'a, T: 'a>(
    fun: impl Fn(Token<'a>) -> Option<T> + 'a,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], T> + 'a {
    map_res(token, move |tok| {
        fun(tok).ok_or(nom::error::ErrorKind::Fail)
    })
}

fn expect<'a>(tk: Token<'a>) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], ()> + 'a {
    context(
        "expect token",
        map_res(token, move |tok| {
            if tok == tk {
                Ok(())
            } else {
                Err(nom::error::ErrorKind::Fail)
            }
        }),
    )
}

fn expect_literal<'a>() -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], Literal<'a>> + 'a {
    context(
        "expect literal",
        map_opt(token, |tok| match tok {
            Token::Literal(lit) => Some(lit),
            _ => None,
        }),
    )
}

fn expect_op<'a>() -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], Op> + 'a {
    context(
        "expect operator",
        expect_map(|tok| match tok {
            Token::Operator(op) => Some(op),
            _ => None,
        }),
    )
}

fn parse_cond(input: &[u8]) -> IResult<&[u8], Cond> {
    fn parser(input: &[u8]) -> IResult<&[u8], Cond> {
        let (rest, tok) = token(input)?;
        match tok {
            Token::Slash => map(parse_item, |item| Cond::ChildExists(item))(rest),
            Token::Literal(lit) => alt((
                tuple((expect_op(), expect_literal())).map(|(op, val)| {
                    let name = lit.to_string();
                    match op {
                        Op::Equal => Cond::Equal {
                            name,
                            value: val.to_string(),
                        },
                        Op::Match => Cond::Match {
                            name,
                            regex: Regex::new(&val.to_string()).unwrap(),
                        },
                    }
                }),
                expect(Token::RightBracket)
                    .map(|_| Cond::Exists(Regex::new(&lit.to_string()).unwrap())),
            ))(rest),
            _ => fail(input),
        }
    }

    context(
        "parse cond",
        delimited(
            expect(Token::LeftBracket),
            parser,
            expect(Token::RightBracket),
        ),
    )(input)
}

fn parse_item(input: &[u8]) -> IResult<&[u8], Item> {
    let parse_filter = map(expect_literal(), |lit| {
        Box::new(Filter::Re(Regex::new(&lit.to_string()).unwrap()))
    });
    context(
        "parse item",
        map(tuple((parse_filter, opt(parse_cond))), |(filter, cond)| {
            Item {
                filter,
                cond: cond.map(Box::new),
            }
        }),
    )(input)
}

fn parse_cpath(mut input: &[u8]) -> IResult<&[u8], CPathBuf> {
    let opt_item = &mut alt((
        expect(Token::DoubleSlash).map(|_| {
            Some(Item {
                filter: Filter::AnyLevel.into(),
                cond: None,
            })
        }),
        map(tuple((expect(Token::Slash), opt(parse_item))), |x| x.1),
        parse_item.map(|x| Some(x)),
        expect(Token::Eof).map(|_| None),
    ));

    let mut res = vec![];
    loop {
        let (rest, item) = opt_item(input)?;
        input = rest;

        if let Some(item) = item {
            res.push(item);
        } else {
            break;
        }
    }

    Ok((input, CPathBuf(res)))
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
