#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Literal<'a> {
    pub raw: &'a str,
    pub quote: u8,
}

impl<'a> std::fmt::Display for Literal<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Into::<String>::into(*self))
    }
}

// TODO: performance

impl<'a> From<Literal<'a>> for String {
    fn from(value: Literal<'a>) -> Self {
        let Literal { raw, quote } = value;
        let mut s = raw.replace(r#"\\"#, "\\");
        if quote == b'"' {
            s = s.replace(r#"\""#, "\"");
        } else if quote == b'\'' {
            s = s.replace(r#"\'"#, "'");
        }
        s.replace("\\\n", "\n")
    }
}