use nom::character::streaming::anychar;
#[allow(unused_imports)]
use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_till, take_until, take_while, take_while_m_n},
    character::{
        complete::{alphanumeric1, char as cchar, multispace0, multispace1, none_of, one_of},
        is_alphabetic, is_newline, is_space,
        streaming::space1,
    },
    combinator::{fail, map, map_res, opt, value},
    error::VerboseError,
    number::complete::be_u8,
    sequence::{delimited, tuple},
};

use crate::lexer::Literal;

pub type IResult<I, O> = nom::IResult<I, O, VerboseError<I>>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Token<'a> {
    Semicolon,
    BlockStart,
    BlockEnd,
    NewLine,
    Eof,
    Literal(Literal<'a>),
}

impl<'a> Token<'a> {
    pub fn ident(&self) -> Option<&'a str> {
        // TODO: check ident rule
        self.raw_string()
    }

    pub fn raw_string(&self) -> Option<&'a str> {
        self.literal().map(|l| l.raw)
    }

    pub fn literal(&self) -> Option<Literal<'a>> {
        match self {
            Self::Literal(l) => Some(*l),
            _ => None,
        }
    }
}

fn starttag(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::BlockStart, tag(b"{"))(input)
}

fn endtag(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::BlockEnd, tag(b"}"))(input)
}

// fn newline(input: &[u8]) -> IResult<&[u8], Token> {
//     value(Token::NewLine, tuple((opt(tag(b"\r")), tag(b"\n"))))(input)
// }

fn semicolon(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::Semicolon, tag(b";"))(input)
}

fn comment(input: &[u8]) -> IResult<&[u8], &[u8]> {
    map(
        tuple((
            tag("#"),
            take_till(is_newline),
            opt(tag(b"\r")),
            opt(tag(b"\n")),
        )),
        |x| x.1,
    )(input)
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
                escaped(none_of(" \t\r\n;'\"\\"), '\\', anychar),
                std::str::from_utf8,
            )(input)
        }
    }?;
    Ok((input, Token::Literal(Literal { raw, quote: first })))
}

pub fn tokenizer(mut input: &[u8]) -> IResult<&[u8], Token> {
    loop {
        let (rest, cmt) = space_and_comment(input)?;
        input = rest;
        if cmt.is_none() {
            break;
        }
    }

    if input.len() == 0 {
        return Ok((input, Token::Eof));
    }

    alt((starttag, endtag, semicolon, literal))(input)
}

fn space_and_comment(input: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
    map(tuple((multispace0, opt(comment))), |x| x.1)(input)
}
