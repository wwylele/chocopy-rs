use super::token::*;
use std::cmp::Ordering;
use std::future::*;

// Adapter that preprocess the input character string:
//  - Attach row and column information to each character
//  - Allow peeking the current character without stepping
//  - Normalize all line breaks to '\n'
//  - Make sure there is '\n' before EOF
struct TextReader<GetChar> {
    get_char: GetChar,
    current: Option<char>,
    position: Position,
    previous_position: Position,
    early_eof: bool,
}

impl<GetCharFuture: Future<Output = Option<char>>, GetChar: FnMut() -> GetCharFuture>
    TextReader<GetChar>
{
    async fn new(mut get_char: GetChar) -> TextReader<GetChar> {
        let current = get_char().await;
        let (current, early_eof) = if current.is_none() {
            (Some('\n'), true)
        } else {
            (current, false)
        };
        TextReader {
            get_char,
            current,
            position: Position { row: 1, col: 1 },
            previous_position: Position { row: 0, col: 0 },
            early_eof,
        }
    }

    async fn next(&mut self) {
        self.previous_position = self.position;
        match self.current.take() {
            Some('\n') => {
                self.position.row += 1;
                self.position.col = 1;
                self.current = (self.get_char)().await;
            }
            Some('\r') => {
                self.position.row += 1;
                self.position.col = 1;
                self.current = (self.get_char)().await;
                if self.current == Some('\n') {
                    self.current = (self.get_char)().await;
                }
            }
            None => (),
            _ => {
                self.position.col += 1;
                self.current = if self.early_eof {
                    None
                } else {
                    let c = (self.get_char)().await;
                    if c.is_none() {
                        self.early_eof = true;
                        Some('\n')
                    } else {
                        c
                    }
                };
            }
        }
    }

    fn current_char(&self) -> Option<char> {
        if self.current == Some('\r') {
            Some('\n')
        } else {
            self.current
        }
    }

    fn current_position(&self) -> Position {
        self.position
    }
    fn previous_position(&self) -> Position {
        self.previous_position
    }
}

#[allow(clippy::cognitive_complexity)]
pub async fn lex<
    GetCharFuture: Future<Output = Option<char>>,
    PutTokenFuture: Future<Output = ()>,
>(
    get_char: impl FnMut() -> GetCharFuture,
    mut put_token: impl FnMut(ComplexToken) -> PutTokenFuture,
) {
    let mut reader = TextReader::new(get_char).await;
    let mut put_token = move |token, start, end| {
        put_token(ComplexToken {
            token,
            location: Location { start, end },
        })
    };
    let mut indentation_stack = vec![0];

    while reader.current_char().is_some() {
        // count indentation
        let indentation_begin = reader.current_position();
        let mut indentation: u32 = 0;
        loop {
            match reader.current_char() {
                Some(' ') => indentation += 1,
                Some('\t') => indentation += 8 - indentation % 8,
                _ => break,
            }
            reader.next().await;
        }

        // The reference program does this weird thing. Yes this can lead to col = 0
        let mut indentation_end = reader.current_position();
        indentation_end.col -= 1;

        // Found comment immediately, skip to line break
        if reader.current_char() == Some('#') {
            while reader.current_char() != Some('\n') {
                reader.next().await;
            }
        }

        // Found line break immediately. This is an empty line
        if reader.current_char() == Some('\n') {
            reader.next().await;
            continue;
        }

        // Calculate indentation
        match indentation.cmp(indentation_stack.last().unwrap()) {
            Ordering::Equal => (),
            Ordering::Greater => {
                indentation_stack.push(indentation);
                put_token(Token::Indent, indentation_begin, indentation_end).await;
            }
            Ordering::Less => {
                let mut dedent_count = 0;
                while indentation < *indentation_stack.last().unwrap() {
                    dedent_count += 1;
                    indentation_stack.pop();
                }
                if indentation != *indentation_stack.last().unwrap() {
                    put_token(Token::Badent, indentation_end, indentation_end).await;
                } else {
                    for _ in 0..dedent_count {
                        put_token(Token::Dedent, indentation_end, indentation_end).await;
                    }
                }
            }
        }

        // Parse normal tokens
        while reader.current_char() != Some('\n') {
            let start = reader.current_position();
            match reader.current_char().unwrap() {
                // Skip spaces
                ' ' | '\t' => {
                    while reader.current_char() == Some(' ') || reader.current_char() == Some('\t')
                    {
                        reader.next().await;
                    }
                }

                // Skip comments
                '#' => {
                    while reader.current_char() != Some('\n') {
                        reader.next().await;
                    }
                }

                // Numbers
                '0'..='9' => {
                    let mut s = "".to_owned();
                    while let c @ '0'..='9' = reader.current_char().unwrap() {
                        s.push(c);
                        reader.next().await;
                    }
                    let end = reader.previous_position();
                    match s.parse() {
                        Ok(n) => put_token(Token::Number(n), start, end).await,
                        Err(_) => put_token(Token::BadNumber, start, end).await,
                    }
                }

                // Words
                'a'..='z' | 'A'..='Z' | '_' => {
                    let mut s = "".to_owned();
                    while let c @ 'a'..='z' | c @ 'A'..='Z' | c @ '_' | c @ '0'..='9' =
                        reader.current_char().unwrap()
                    {
                        s.push(c);
                        reader.next().await;
                    }
                    let end = reader.previous_position();
                    put_token(
                        KEYWORDS
                            .get(&s[..])
                            .cloned()
                            .unwrap_or_else(|| Token::Identifier(s)),
                        start,
                        end,
                    )
                    .await;
                }

                // Strings
                '\"' => {
                    reader.next().await;
                    let mut s = "".to_owned();
                    let mut is_id = true;
                    loop {
                        match reader.current_char().unwrap() {
                            // end quote
                            '\"' => {
                                reader.next().await;
                                break;
                            }
                            // escape
                            '\\' => {
                                is_id = false;
                                reader.next().await;
                                match reader.current_char().unwrap() {
                                    'n' => s.push('\n'),
                                    't' => s.push('\t'),
                                    '\\' => s.push('\\'),
                                    '\"' => s.push('\"'),
                                    c => {
                                        reader.next().await;
                                        put_token(
                                            Token::Unrecognized(c.to_string()),
                                            start,
                                            reader.previous_position(),
                                        )
                                        .await;
                                        break;
                                    }
                                }
                            }
                            // normal char
                            c @ ' '..='~' => {
                                if let 'a'..='z' | 'A'..='Z' | '_' | '0'..='9' = c {
                                } else {
                                    is_id = false;
                                }
                                s.push(c);
                            }
                            // unrecognized
                            c => {
                                reader.next().await;
                                put_token(
                                    Token::Unrecognized(c.to_string()),
                                    start,
                                    reader.previous_position(),
                                )
                                .await;
                                break;
                            }
                        }
                        reader.next().await;
                    }
                    let end = reader.previous_position();
                    if let Some('0'..='9') | None = s.chars().next() {
                        is_id = false;
                    }
                    put_token(
                        if is_id {
                            Token::IdString(s)
                        } else {
                            Token::StringLiteral(s)
                        },
                        start,
                        end,
                    )
                    .await;
                }

                // Operators
                c => {
                    reader.next().await;

                    let token = if let Some(operator) = OPERATORS.get(&c) {
                        let second = reader.current_char().unwrap();
                        if let Some(operator) = operator.get(&second) {
                            reader.next().await;
                            operator.clone()
                        } else if let Some(operator) = operator.get(&'\0') {
                            operator.clone()
                        } else {
                            Token::Unrecognized(c.to_string())
                        }
                    } else {
                        Token::Unrecognized(c.to_string())
                    };
                    put_token(token, start, reader.previous_position()).await;
                }
            }
        }

        // Finish the line
        let new_line_begin = reader.current_position();
        put_token(Token::NewLine, new_line_begin, new_line_begin).await;
        reader.next().await;
    }

    let mut end = reader.current_position();

    // Last dedent
    for _ in 1..indentation_stack.len() {
        put_token(Token::Dedent, end, end).await;
        end.col += 1; // The reference program does this weird thing
    }

    put_token(Token::Eof, end, end).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use futures::future::*;

    struct StrGetChar<'a> {
        iter: std::str::Chars<'a>,
    }

    impl<'a> StrGetChar<'a> {
        fn get(&mut self) -> Ready<Option<char>> {
            ready(self.iter.next())
        }
    }

    fn str_get_char<'a>(s: &'a str) -> impl FnMut() -> Ready<Option<char>> + 'a {
        let mut sgc = StrGetChar::<'a> { iter: s.chars() };
        move || sgc.get()
    }

    fn read_all(s: &str) -> Vec<(char, Position)> {
        block_on(async {
            let mut reader = TextReader::new(str_get_char(s)).await;
            let mut v = vec![];
            loop {
                let c = reader.current_char();
                if let Some(c) = c {
                    v.push((c, reader.current_position()));
                    reader.next().await;
                } else {
                    break v;
                }
            }
        })
    }

    #[test]
    fn text_reader() {
        assert_eq!(
            read_all("a"),
            vec![
                ('a', Position { row: 1, col: 1 }),
                ('\n', Position { row: 1, col: 2 })
            ]
        );

        assert_eq!(
            read_all("b\r"),
            vec![
                ('b', Position { row: 1, col: 1 }),
                ('\n', Position { row: 1, col: 2 })
            ]
        );

        assert_eq!(
            read_all("c\r\n"),
            vec![
                ('c', Position { row: 1, col: 1 }),
                ('\n', Position { row: 1, col: 2 })
            ]
        );

        assert_eq!(
            read_all("d\n"),
            vec![
                ('d', Position { row: 1, col: 1 }),
                ('\n', Position { row: 1, col: 2 })
            ]
        );
        assert_eq!(read_all(""), vec![('\n', Position { row: 1, col: 1 })]);
        assert_eq!(read_all("\r"), vec![('\n', Position { row: 1, col: 1 })]);
        assert_eq!(read_all("\r\n"), vec![('\n', Position { row: 1, col: 1 })]);
        assert_eq!(read_all("\n"), vec![('\n', Position { row: 1, col: 1 })]);

        assert_eq!(
            read_all("a\n\rb\r\n\rc"),
            vec![
                ('a', Position { row: 1, col: 1 }),
                ('\n', Position { row: 1, col: 2 }),
                ('\n', Position { row: 2, col: 1 }),
                ('b', Position { row: 3, col: 1 }),
                ('\n', Position { row: 3, col: 2 }),
                ('\n', Position { row: 4, col: 1 }),
                ('c', Position { row: 5, col: 1 }),
                ('\n', Position { row: 5, col: 2 }),
            ]
        );
    }

    fn lex_case(s: &str, tokens_ref: &[Token]) {
        use std::cell::*;
        use std::rc::*;
        let tokens = Rc::new(RefCell::new(vec![]));
        let put_token = {
            let tokens = tokens.clone();
            move |complex_token: ComplexToken| {
                tokens.borrow_mut().push(complex_token.token);
                async {}
            }
        };

        let get_char = str_get_char(s);

        block_on(lex(get_char, put_token));
        assert_eq!(&tokens.borrow()[..], tokens_ref);
    }

    #[test]
    fn lex_test() {
        lex_case("3", &[Token::Number(3), Token::NewLine, Token::Eof]);
        lex_case(
            "abc",
            &[
                Token::Identifier("abc".to_owned()),
                Token::NewLine,
                Token::Eof,
            ],
        );

        #[rustfmt::skip]
        lex_case(
            "
>1+ 3<=5  #Hello\"
   # World!
a _b \t x2
   d else\t
       f
 \t

           42  \"xyz_3\"
   l
   \t \"p\\nl\\\"123\" 66
   \t q
",
            &[
    Token::Greater, Token::Number(1), Token::Plus, Token::Number(3), Token::LessEqual, Token::Number(5), Token::NewLine,
    Token::Identifier("a".to_owned()), Token::Identifier("_b".to_owned()), Token::Identifier("x2".to_owned()), Token::NewLine,
    Token::Indent, Token::Identifier("d".to_owned()), Token::Else, Token::NewLine,
    Token::Indent, Token::Identifier("f".to_owned()), Token::NewLine,
    Token::Indent, Token::Number(42), Token::IdString("xyz_3".to_owned()), Token::NewLine,
    Token::Dedent, Token::Dedent, Token::Identifier("l".to_owned()), Token::NewLine,
    Token::Indent, Token::StringLiteral("p\nl\"123".to_owned()), Token::Number(66), Token::NewLine,
    Token::Identifier("q".to_owned()), Token::NewLine,
    Token::Dedent, Token::Dedent, Token::Eof
        ]);
    }
}
