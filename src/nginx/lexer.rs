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
    Comment(Literal<'a>),
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
                escaped(none_of("{ \t\r\n;'\"\\"), '\\', anychar),
                std::str::from_utf8,
            )(input)
        }
    }?;
    Ok((input, Token::Literal(Literal { raw, quote: first })))
}

fn tocomment(input: &[u8]) -> IResult<&[u8], Token> {
    map(tuple((tag("#"), take_till(is_newline))), |x| {
        Token::Comment(Literal {
            raw: std::str::from_utf8(x.1).unwrap_or_default(),
            quote: b'#',
        })
    })(input)
}

pub fn tokenizer(mut input: &[u8]) -> IResult<&[u8], Token> {
    loop {
        let (rest, cmt) = multispace0(input)?;
        // println!("{cmt:?}");
        // if let Some(c) = cmt.clone() {
        //     // println!("{}", String::from_utf8_lossy(rest).to_string());
        //     println!("{}", String::from_utf8(c).unwrap_or_default());
        // }
        // println!("{} {} {} - {:?}", rest.len(), cmt.len(), input.len(), cmt);
        // println!("{}", String::from_utf8_lossy(cmt).to_string());
        input = rest;
        if cmt.len() == 0 {
            break;
        }
    }

    // println!("{}", String::from_utf8_lossy(input).to_string());

    if input.len() == 0 {
        return Ok((input, Token::Eof));
    }

    alt((starttag, endtag, tocomment, semicolon, literal))(input)
}
