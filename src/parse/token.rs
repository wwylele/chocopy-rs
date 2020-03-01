use crate::location::*;
use lazy_static::*;
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ComplexToken {
    pub token: Token,
    pub location: Location,
}
