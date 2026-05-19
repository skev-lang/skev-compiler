#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Entity,
    When,
    Fn,
    Data,
    Kind,
    Match,
    If,
    Else,
    Loop,
    Task,
    Async,
    Await,
    Has,
    Import,
    Extern,
    Result,
    Fail,
    Succeed,
    Maybe,
    Shared,
    Realtime,
    Unsafe,
    Test,
    Mock,
    Every,
    Event,

    // Operators
    RightArrow,  // ->
    FatArrow,    // =>
    OpenBlock,   // >>
    CloseBlock,  // <<
    ColonColon,  // ::
    Colon,       // :
    PlusEq,      // +=
    MinusEq,     // -=
    StarEq,      // *=
    SlashEq,     // /=
    EqEq,        // ==
    NotEq,       // !=
    LtEq,        // <=
    GtEq,        // >=
    Lt,          // <
    Gt,          // >
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /

    // Wrapping operators
    WrapAdd,           // +%
    WrapSub,           // -%
    WrapMul,           // *%
    WrapAddEq,         // +%=
    WrapSubEq,         // -%=
    WrapMulEq,         // *%=

    // Saturating operators
    SatAdd,            // +|
    SatSub,            // -|
    SatMul,            // *|
    SatAddEq,          // +|=
    SatSubEq,          // -|=
    SatMulEq,          // *|=

    // Always-panic operators
    PanicAdd,          // +!
    PanicSub,          // -!
    PanicMul,          // *!
    PanicAddEq,        // +!=
    PanicSubEq,        // -!=
    PanicMulEq,        // *!=
    Eq,          // =
    Dot,         // .
    Comma,       // ,
    Bang,        // !
    Question,    // ?

    // Delimiters
    LParen,      // (
    RParen,      // )
    LBracket,    // [
    RBracket,    // ]

    // Literals
    IntLiteral(i64),
    FloatLiteral(f64),
    StringStart,
    StringContent(String),
    InterpolationExpr(String),
    StringEnd,
    BoolLiteral(bool),

    // Identifiers
    Identifier(String),

    // Comments
    DocComment(String), // #! only

    // End
    EOF,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn push(&mut self, kind: TokenKind, line: usize, col: usize) {
        self.tokens.push(Token { kind, line, col });
    }

    fn err(&mut self, message: String, line: usize, col: usize) {
        self.errors.push(LexError { message, line, col });
    }

    fn run(&mut self) {
        while self.pos < self.chars.len() {
            self.next_token();
        }
        self.push(TokenKind::EOF, self.line, self.col);
    }

    fn next_token(&mut self) {
        let c = match self.peek() {
            Some(c) => c,
            None => return,
        };

        if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
            self.advance();
            return;
        }

        if c == '#' {
            self.lex_comment();
            return;
        }

        if c == '"' {
            self.lex_string();
            return;
        }

        if c.is_ascii_digit() {
            self.lex_number();
            return;
        }

        if c.is_ascii_alphabetic() || c == '_' {
            self.lex_identifier();
            return;
        }

        self.lex_operator();
    }

    fn lex_comment(&mut self) {
        let line = self.line;
        let col = self.col;
        self.advance(); // consume '#'

        if self.peek() == Some('!') {
            self.advance(); // consume '!'
            let mut content = String::new();
            while let Some(c) = self.peek() {
                if c == '\n' {
                    break;
                }
                content.push(c);
                self.advance();
            }
            self.push(TokenKind::DocComment(content), line, col);
        } else {
            while let Some(c) = self.peek() {
                if c == '\n' {
                    break;
                }
                self.advance();
            }
        }
    }

    fn lex_string(&mut self) {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // consume opening '"'
        self.push(TokenKind::StringStart, start_line, start_col);

        let mut content = String::new();
        let mut content_line = self.line;
        let mut content_col = self.col;

        loop {
            match self.peek() {
                None => {
                    if !content.is_empty() {
                        self.push(
                            TokenKind::StringContent(std::mem::take(&mut content)),
                            content_line,
                            content_col,
                        );
                    }
                    self.err(
                        "Unterminated string literal".to_string(),
                        start_line,
                        start_col,
                    );
                    return;
                }
                Some('"') => {
                    if !content.is_empty() {
                        self.push(
                            TokenKind::StringContent(std::mem::take(&mut content)),
                            content_line,
                            content_col,
                        );
                    }
                    let el = self.line;
                    let ec = self.col;
                    self.advance(); // consume closing '"'
                    self.push(TokenKind::StringEnd, el, ec);
                    return;
                }
                Some('{') => {
                    if !content.is_empty() {
                        self.push(
                            TokenKind::StringContent(std::mem::take(&mut content)),
                            content_line,
                            content_col,
                        );
                    }
                    let il = self.line;
                    let ic = self.col;
                    self.advance(); // consume '{'
                    let mut expr = String::new();
                    let mut depth: usize = 1;
                    loop {
                        match self.peek() {
                            None => {
                                self.err(
                                    "Unterminated interpolation expression".to_string(),
                                    il,
                                    ic,
                                );
                                self.push(TokenKind::InterpolationExpr(expr), il, ic);
                                return;
                            }
                            Some('{') => {
                                depth += 1;
                                expr.push('{');
                                self.advance();
                            }
                            Some('}') => {
                                depth -= 1;
                                if depth == 0 {
                                    self.advance();
                                    break;
                                } else {
                                    expr.push('}');
                                    self.advance();
                                }
                            }
                            Some(ch) => {
                                expr.push(ch);
                                self.advance();
                            }
                        }
                    }
                    self.push(TokenKind::InterpolationExpr(expr), il, ic);
                    content_line = self.line;
                    content_col = self.col;
                }
                Some(ch) => {
                    if content.is_empty() {
                        content_line = self.line;
                        content_col = self.col;
                    }
                    content.push(ch);
                    self.advance();
                }
            }
        }
    }

    fn lex_number(&mut self) {
        let line = self.line;
        let col = self.col;
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let is_float = self.peek() == Some('.')
            && self.peek_at(1).map_or(false, |c| c.is_ascii_digit());

        if is_float {
            s.push('.');
            self.advance(); // consume '.'
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            match s.parse::<f64>() {
                Ok(f) => self.push(TokenKind::FloatLiteral(f), line, col),
                Err(_) => self.err(format!("Invalid float literal: {}", s), line, col),
            }
        } else {
            match s.parse::<i64>() {
                Ok(i) => self.push(TokenKind::IntLiteral(i), line, col),
                Err(_) => self.err(format!("Invalid integer literal: {}", s), line, col),
            }
        }
    }

    fn lex_identifier(&mut self) {
        let line = self.line;
        let col = self.col;
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match s.as_str() {
            "entity" => TokenKind::Entity,
            "when" => TokenKind::When,
            "fn" => TokenKind::Fn,
            "data" => TokenKind::Data,
            "kind" => TokenKind::Kind,
            "match" => TokenKind::Match,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "loop" => TokenKind::Loop,
            "task" => TokenKind::Task,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "has" => TokenKind::Has,
            "import" => TokenKind::Import,
            "extern" => TokenKind::Extern,
            "result" => TokenKind::Result,
            "fail" => TokenKind::Fail,
            "succeed" => TokenKind::Succeed,
            "maybe" => TokenKind::Maybe,
            "shared" => TokenKind::Shared,
            "realtime" => TokenKind::Realtime,
            "unsafe" => TokenKind::Unsafe,
            "test" => TokenKind::Test,
            "mock" => TokenKind::Mock,
            "every" => TokenKind::Every,
            "event" => TokenKind::Event,
            "true" => TokenKind::BoolLiteral(true),
            "false" => TokenKind::BoolLiteral(false),
            _ => TokenKind::Identifier(s),
        };

        self.push(kind, line, col);
    }

    fn lex_operator(&mut self) {
        let line = self.line;
        let col = self.col;
        let c = match self.peek() {
            Some(c) => c,
            None => return,
        };
        let n = self.peek_at(1);

        let matched: Option<(TokenKind, usize)> = match c {
            '>' => Some(match n {
                Some('>') => (TokenKind::OpenBlock, 2),
                Some('=') => (TokenKind::GtEq, 2),
                _ => (TokenKind::Gt, 1),
            }),
            '<' => Some(match n {
                Some('<') => (TokenKind::CloseBlock, 2),
                Some('=') => (TokenKind::LtEq, 2),
                _ => (TokenKind::Lt, 1),
            }),
            '-' => Some(match (n, self.peek_at(2)) {
                (Some('%'), Some('=')) => (TokenKind::WrapSubEq, 3),
                (Some('|'), Some('=')) => (TokenKind::SatSubEq, 3),
                (Some('!'), Some('=')) => (TokenKind::PanicSubEq, 3),
                (Some('%'), _) => (TokenKind::WrapSub, 2),
                (Some('|'), _) => (TokenKind::SatSub, 2),
                (Some('!'), _) => (TokenKind::PanicSub, 2),
                (Some('>'), _) => (TokenKind::RightArrow, 2),
                (Some('='), _) => (TokenKind::MinusEq, 2),
                _ => (TokenKind::Minus, 1),
            }),
            '=' => Some(match n {
                Some('=') => (TokenKind::EqEq, 2),
                Some('>') => (TokenKind::FatArrow, 2),
                _ => (TokenKind::Eq, 1),
            }),
            '!' => Some(match n {
                Some('=') => (TokenKind::NotEq, 2),
                _ => (TokenKind::Bang, 1),
            }),
            ':' => Some(match n {
                Some(':') => (TokenKind::ColonColon, 2),
                _ => (TokenKind::Colon, 1),
            }),
            '+' => Some(match (n, self.peek_at(2)) {
                (Some('%'), Some('=')) => (TokenKind::WrapAddEq, 3),
                (Some('|'), Some('=')) => (TokenKind::SatAddEq, 3),
                (Some('!'), Some('=')) => (TokenKind::PanicAddEq, 3),
                (Some('%'), _) => (TokenKind::WrapAdd, 2),
                (Some('|'), _) => (TokenKind::SatAdd, 2),
                (Some('!'), _) => (TokenKind::PanicAdd, 2),
                (Some('='), _) => (TokenKind::PlusEq, 2),
                _ => (TokenKind::Plus, 1),
            }),
            '*' => Some(match (n, self.peek_at(2)) {
                (Some('%'), Some('=')) => (TokenKind::WrapMulEq, 3),
                (Some('|'), Some('=')) => (TokenKind::SatMulEq, 3),
                (Some('!'), Some('=')) => (TokenKind::PanicMulEq, 3),
                (Some('%'), _) => (TokenKind::WrapMul, 2),
                (Some('|'), _) => (TokenKind::SatMul, 2),
                (Some('!'), _) => (TokenKind::PanicMul, 2),
                (Some('='), _) => (TokenKind::StarEq, 2),
                _ => (TokenKind::Star, 1),
            }),
            '/' => Some(match n {
                Some('=') => (TokenKind::SlashEq, 2),
                _ => (TokenKind::Slash, 1),
            }),
            '.' => Some((TokenKind::Dot, 1)),
            ',' => Some((TokenKind::Comma, 1)),
            '?' => Some((TokenKind::Question, 1)),
            '(' => Some((TokenKind::LParen, 1)),
            ')' => Some((TokenKind::RParen, 1)),
            '[' => Some((TokenKind::LBracket, 1)),
            ']' => Some((TokenKind::RBracket, 1)),
            _ => None,
        };

        match matched {
            Some((kind, len)) => {
                for _ in 0..len {
                    self.advance();
                }
                self.push(kind, line, col);
            }
            None => {
                self.err(format!("Unexpected character: '{}'", c), line, col);
                self.advance();
            }
        }
    }
}

pub fn lex(source: &str) -> (Vec<Token>, Vec<LexError>) {
    let mut lx = Lexer::new(source);
    lx.run();
    (lx.tokens, lx.errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        let (tokens, errors) = lex("entity when fn");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::Entity));
        assert!(matches!(tokens[1].kind, TokenKind::When));
        assert!(matches!(tokens[2].kind, TokenKind::Fn));
    }

    #[test]
    fn test_operators() {
        let (tokens, errors) = lex(">> << :: ->");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::OpenBlock));
        assert!(matches!(tokens[1].kind, TokenKind::CloseBlock));
        assert!(matches!(tokens[2].kind, TokenKind::ColonColon));
        assert!(matches!(tokens[3].kind, TokenKind::RightArrow));
    }

    #[test]
    fn test_integer_literal() {
        let (tokens, errors) = lex("42");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::IntLiteral(42)));
    }

    #[test]
    fn test_float_literal() {
        let (tokens, errors) = lex("3.14");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::FloatLiteral(_)));
    }

    #[test]
    fn test_bool_literals() {
        let (tokens, errors) = lex("true false");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::BoolLiteral(true)));
        assert!(matches!(tokens[1].kind, TokenKind::BoolLiteral(false)));
    }

    #[test]
    fn test_comment_discarded() {
        let (tokens, errors) = lex("# this is a comment\nentity");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::Entity));
    }

    #[test]
    fn test_doc_comment_kept() {
        let (tokens, errors) = lex("#! this is a doc comment");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::DocComment(_)));
    }

    #[test]
    fn test_unknown_char_error() {
        let (_, errors) = lex("@");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].line, 1);
    }

    #[test]
    fn test_identifier() {
        let (tokens, errors) = lex("PlayerCar");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(_)));
    }

    #[test]
    fn test_string_basic() {
        let (tokens, errors) = lex("\"hello\"");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::StringStart));
        assert!(matches!(tokens[1].kind, TokenKind::StringContent(_)));
        assert!(matches!(tokens[2].kind, TokenKind::StringEnd));
    }

    #[test]
    fn test_wrapping_operators() {
        let (tokens, errors) = lex("+% -% *%");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::WrapAdd));
        assert!(matches!(tokens[1].kind, TokenKind::WrapSub));
        assert!(matches!(tokens[2].kind, TokenKind::WrapMul));
    }

    #[test]
    fn test_saturating_operators() {
        let (tokens, errors) = lex("+| -| *|");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::SatAdd));
        assert!(matches!(tokens[1].kind, TokenKind::SatSub));
        assert!(matches!(tokens[2].kind, TokenKind::SatMul));
    }

    #[test]
    fn test_panic_operators() {
        let (tokens, errors) = lex("+! -! *!");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::PanicAdd));
        assert!(matches!(tokens[1].kind, TokenKind::PanicSub));
        assert!(matches!(tokens[2].kind, TokenKind::PanicMul));
    }

    #[test]
    fn test_augmented_wrapping() {
        let (tokens, errors) = lex("+%= -%= *%=");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::WrapAddEq));
        assert!(matches!(tokens[1].kind, TokenKind::WrapSubEq));
        assert!(matches!(tokens[2].kind, TokenKind::WrapMulEq));
    }

    #[test]
    fn test_augmented_saturating() {
        let (tokens, errors) = lex("+|= -|= *|=");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::SatAddEq));
        assert!(matches!(tokens[1].kind, TokenKind::SatSubEq));
        assert!(matches!(tokens[2].kind, TokenKind::SatMulEq));
    }

    #[test]
    fn test_augmented_panic() {
        let (tokens, errors) = lex("+!= -!= *!=");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::PanicAddEq));
        assert!(matches!(tokens[1].kind, TokenKind::PanicSubEq));
        assert!(matches!(tokens[2].kind, TokenKind::PanicMulEq));
    }

    #[test]
    fn test_panic_assign_vs_not_equal() {
        // +!= must lex as ONE token (PanicAddEq)
        // NOT as Plus + NotEq
        let (tokens, errors) = lex("x +!= y");
        assert!(errors.is_empty());
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(_)));
        assert!(matches!(tokens[1].kind, TokenKind::PanicAddEq));
        assert!(matches!(tokens[2].kind, TokenKind::Identifier(_)));
        assert_eq!(tokens.len(), 4); // x, +!=, y, EOF
    }

    #[test]
    fn test_panic_op_then_not_equal() {
        // +! followed by != must lex as TWO tokens
        let (tokens, errors) = lex("x +! y != z");
        assert!(errors.is_empty());
        assert!(matches!(tokens[1].kind, TokenKind::PanicAdd));
        assert!(matches!(tokens[3].kind, TokenKind::NotEq));
    }
}
