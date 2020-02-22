use lazy_static::*;
use serde::{de::*, ser::*};
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Token {
    NewLine,
    Indent,
    Dedent,
    Badent,
    Number(i32),
    BadNumber,
    Identifier(String),
    IdString(String),
    StringLiteral(String),

    False,
    None,
    True,
    And,
    As,
    Assert,
    Async,
    Await,
    Break,
    Class,
    Continue,
    Def,
    Del,
    Elif,
    Else,
    Except,
    Finally,
    For,
    From,
    Global,
    If,
    Import,
    In,
    Is,
    Lambda,
    Nonlocal,
    Not,
    Or,
    Pass,
    Raise,
    Return,
    Try,
    While,
    With,
    Yield,

    Plus,
    Minus,
    Multiply,
    Divide,
    Mod,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    NotEqual,
    Assign,
    LeftPar,
    RightPar,
    LeftSquare,
    RightSquare,
    Comma,
    Colon,
    Dot,
    Arrow,

    Unrecognized(String),
    Eof,
}

lazy_static! {
    pub static ref KEYWORDS: HashMap<&'static str, Token> = vec![
        ("False", Token::False),
        ("None", Token::None),
        ("True", Token::True),
        ("and", Token::And),
        ("as", Token::As),
        ("assert", Token::Assert),
        ("async", Token::Async),
        ("await", Token::Await),
        ("break", Token::Break),
        ("class", Token::Class),
        ("continue", Token::Continue),
        ("def", Token::Def),
        ("del", Token::Del),
        ("elif", Token::Elif),
        ("else", Token::Else),
        ("except", Token::Except),
        ("finally", Token::Finally),
        ("for", Token::For),
        ("from", Token::From),
        ("global", Token::Global),
        ("if", Token::If),
        ("import", Token::Import),
        ("in", Token::In),
        ("is", Token::Is),
        ("lambda", Token::Lambda),
        ("nonlocal", Token::Nonlocal),
        ("not", Token::Not),
        ("or", Token::Or),
        ("pass", Token::Pass),
        ("raise", Token::Raise),
        ("return", Token::Return),
        ("try", Token::Try),
        ("while", Token::While),
        ("with", Token::With),
        ("yield", Token::Yield),
    ]
    .into_iter()
    .collect();
    pub static ref OPERATORS: HashMap<char, HashMap<char, Token>> = vec![
        ('+', vec![('\0', Token::Plus)].into_iter().collect()),
        (
            '-',
            vec![('\0', Token::Minus), ('>', Token::Arrow)]
                .into_iter()
                .collect()
        ),
        ('*', vec![('\0', Token::Multiply)].into_iter().collect()),
        ('/', vec![('/', Token::Divide)].into_iter().collect()),
        ('%', vec![('\0', Token::Mod)].into_iter().collect()),
        (
            '<',
            vec![('\0', Token::Less), ('=', Token::LessEqual)]
                .into_iter()
                .collect()
        ),
        (
            '>',
            vec![('\0', Token::Greater), ('=', Token::GreaterEqual)]
                .into_iter()
                .collect()
        ),
        (
            '=',
            vec![('\0', Token::Assign), ('=', Token::Equal)]
                .into_iter()
                .collect()
        ),
        ('!', vec![('=', Token::NotEqual)].into_iter().collect()),
        ('(', vec![('\0', Token::LeftPar)].into_iter().collect()),
        (')', vec![('\0', Token::RightPar)].into_iter().collect()),
        ('[', vec![('\0', Token::LeftSquare)].into_iter().collect()),
        (']', vec![('\0', Token::RightSquare)].into_iter().collect()),
        (',', vec![('\0', Token::Comma)].into_iter().collect()),
        (':', vec![('\0', Token::Colon)].into_iter().collect()),
        ('.', vec![('\0', Token::Dot)].into_iter().collect()),
    ]
    .into_iter()
    .collect();
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    pub row: u32,
    pub col: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location {
    pub start: Position,
    pub end: Position,
}

impl Location {
    pub fn new(sr: u32, sc: u32, er: u32, ec: u32) -> Location {
        Location {
            start: Position { row: sr, col: sc },
            end: Position { row: er, col: ec },
        }
    }
}

impl Serialize for Location {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let array = [self.start.row, self.start.col, self.end.row, self.end.col];
        array.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Location {
    fn deserialize<D>(deserializer: D) -> Result<Location, D::Error>
    where
        D: Deserializer<'de>,
    {
        let array = <[u32; 4] as Deserialize<'de>>::deserialize(deserializer)?;
        Ok(Location {
            start: Position {
                row: array[0],
                col: array[1],
            },
            end: Position {
                row: array[2],
                col: array[3],
            },
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ComplexToken {
    pub token: Token,
    pub location: Location,
}
