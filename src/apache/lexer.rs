use nom::character::{complete::space0, streaming::anychar};
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
    number::complete::be_u8,
    sequence::{delimited, tuple},
    IResult,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Token<'a> {
    OpenTag,
    EndTag,
    CloseTag,
    NewLine,
    Eof,
    Literal { raw: &'a str, quote: u8 },
}

impl<'a> Token<'a> {
    pub fn ident(&self) -> Option<&'a str> {
        // TODO: check ident rule
        self.raw_string()
    }

    pub fn raw_string(&self) -> Option<&'a str> {
        match self {
            Self::Literal { raw, .. } => Some(*raw),
            _ => None,
        }
    }

    pub fn unescape(&self) -> Option<String> {
        Some(match self {
            Self::Literal { raw, quote } => {
                let s = raw.replace(r#"\\"#, "\\");
                if *quote == b'"' {
                    s.replace(r#"\""#, "\"")
                } else if *quote == b'\'' {
                    s.replace(r#"\'"#, "'")
                } else {
                    s.replace("\\\n", "\n")
                }
            }
            _ => return None,
        })
    }
}

fn opentag(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::OpenTag, tag(b"<"))(input)
}

fn endtag(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::EndTag, tag(b">"))(input)
}

fn newline(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::NewLine, tuple((opt(tag(b"\r")), tag(b"\n"))))(input)
}

fn closetag(input: &[u8]) -> IResult<&[u8], Token> {
    value(Token::CloseTag, tag(b"</"))(input)
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
                escaped(none_of(" \t\r\n<>'\"\\"), '\\', anychar),
                std::str::from_utf8,
            )(input)
        }
    }?;
    Ok((input, Token::Literal { raw, quote: first }))
}

pub fn tokenizer(input: &[u8]) -> IResult<&[u8], Token> {
    inner_tokenizer::<false>(input)
}

fn space_and_comment<const NL: bool>(input: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
    let space = if NL { space0 } else { multispace0 };
    map(tuple((space, opt(comment))), |x| x.1)(input)
}

pub fn inner_tokenizer<const NL: bool>(mut input: &[u8]) -> IResult<&[u8], Token> {
    loop {
        let (rest, cmt) = space_and_comment::<NL>(input)?;
        input = rest;
        if cmt.is_some() {
            // println!("[cmt] {:?}", std::str::from_utf8(cmt.unwrap()));
        } else {
            break;
        }
    }
    if input.len() == 0 {
        return Ok((input, Token::Eof));
    }
    alt((closetag, opentag, endtag, newline, literal))(input)
}
