use crate::lexer::{Token, TokenKind};

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    Entity {
        name: String,
        body: Vec<EntityItem>,
    },
    Fn {
        name: String,
        params: Vec<Param>,
        ret: Option<TypeExpr>,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Data {
        name: String,
        fields: Vec<Field>,
    },
    Kind {
        name: String,
        variants: Vec<KindVariant>,
    },
    Import(String),
    Extern {
        lang: String,
        name: String,
        items: Vec<ExternItem>,
    },
    Test {
        description: String,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Mock {
        target: String,
        body: Vec<MockItem>,
    },
}

#[derive(Debug, Clone)]
pub enum EntityItem {
    Property {
        name: String,
        ty: TypeExpr,
        value: Option<Expr>,
        is_shared: bool,
    },
    Has(String),
    When {
        event: String,
        params: Vec<Param>,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Method {
        name: String,
        params: Vec<Param>,
        ret: Option<TypeExpr>,
        body: Vec<Stmt>,
        is_async: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopKind {
    Range,
    Iterate,
    While,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl {
        name: String,
        ty: Option<TypeExpr>,
        value: Option<Expr>,
    },
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },
    ExprStmt(Expr),
    Fail(Expr),
    Succeed(Expr),
    Result(Expr),
    Event(Expr),
    Loop {
        kind: LoopKind,
        var_name: Option<String>,
        from_expr: Option<Expr>,
        to_expr: Option<Expr>,
        iterable: Option<Expr>,
        condition: Option<Expr>,
        body: Vec<Stmt>,
    },
    Stop,
    Skip,
    Every {
        interval: Expr,
        body: Vec<Stmt>,
    },
    If {
        condition: Expr,
        then: Vec<Stmt>,
        else_: Option<Vec<Stmt>>,
    },
    Await(Expr),
    Cancel(String),
    Match {
        subject: Expr,
        arms: Vec<MatchArm>,
    },
    Task {
        body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    StringLiteral(Vec<StringPart>),
    Identifier(String),
    BinaryOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    IfExists {
        value: Box<Expr>,
        binding: String,
        then: Vec<Stmt>,
        else_: Option<Vec<Stmt>>,
    },
    Or {
        value: Box<Expr>,
        fallback: Box<Expr>,
    },
    As {
        value: Box<Expr>,
        ty: TypeExpr,
    },
    ListLiteral(Vec<Expr>),
    MapLiteral(Vec<(Expr, Expr)>),
    Contains {
        collection: Box<Expr>,
        item: Box<Expr>,
    },
    Async(Box<Expr>),
    Arrow(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum StringPart {
    Literal(String),
    Interpolation(Expr),
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Named(String),
    Generic {
        base: String,
        args: Vec<TypeExpr>,
    },
    Maybe(Box<TypeExpr>),
    GameNative(String),
    Result(Box<TypeExpr>),
    Channel(Box<TypeExpr>),
    List(Box<TypeExpr>),
    Array {
        ty: Box<TypeExpr>,
        size: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub label: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct KindVariant {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ExternItem {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct MockItem {
    pub event: String,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum MatchPattern {
    Literal(Expr),
    Identifier(String),
    Wildcard,
    GuardedWildcard(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,

    // Wrapping arithmetic
    WrapAdd,    // +%
    WrapSub,    // -%
    WrapMul,    // *%

    // Saturating arithmetic
    SatAdd,     // +|
    SatSub,     // -|
    SatMul,     // *|

    // Always-panic arithmetic
    PanicAdd,   // +!
    PanicSub,   // -!
    PanicMul,   // *!

    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,

    // Wrapping assign
    WrapAddEq,   // +%=
    WrapSubEq,   // -%=
    WrapMulEq,   // *%=

    // Saturating assign
    SatAddEq,    // +|=
    SatSubEq,    // -|=
    SatMulEq,    // *|=

    // Panic assign
    PanicAddEq,  // +!=
    PanicSubEq,  // -!=
    PanicMulEq,  // *!=
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    fn peek(&self) -> &Token {
        let idx = self.pos.min(self.tokens.len().saturating_sub(1));
        &self.tokens[idx]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_offset(&self, offset: usize) -> &Token {
        let idx = (self.pos + offset).min(self.tokens.len().saturating_sub(1));
        &self.tokens[idx]
    }

    fn advance(&mut self) -> Token {
        let t = self.peek().clone();
        if !matches!(t.kind, TokenKind::EOF) {
            self.pos += 1;
        }
        t
    }

    fn at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::EOF)
    }

    fn error_at_peek(&mut self, msg: String) {
        let p = self.peek();
        self.errors.push(ParseError {
            message: msg,
            line: p.line,
            col: p.col,
        });
    }

    fn expect_open_block(&mut self) -> bool {
        if matches!(self.peek_kind(), TokenKind::OpenBlock) {
            self.advance();
            true
        } else {
            self.error_at_peek("Expected '>>'".to_string());
            false
        }
    }

    fn consume_close(&mut self) -> bool {
        if !matches!(self.peek_kind(), TokenKind::CloseBlock) {
            self.error_at_peek("Expected '<<'".to_string());
            return false;
        }
        let close_line = self.peek().line;
        self.advance();
        while !matches!(
            self.peek_kind(),
            TokenKind::EOF | TokenKind::CloseBlock | TokenKind::OpenBlock
        ) && self.peek().line == close_line
        {
            self.advance();
        }
        true
    }

    fn parse_identifier(&mut self) -> String {
        let kind = self.peek_kind().clone();
        if let TokenKind::Identifier(s) = kind {
            self.advance();
            s
        } else {
            self.error_at_peek("Expected identifier".to_string());
            String::new()
        }
    }

    fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        while !self.at_end() {
            let start = self.pos;
            if let Some(item) = self.parse_top_level() {
                items.push(item);
            }
            if self.pos == start {
                self.advance();
            }
        }
        Program { items }
    }

    fn parse_top_level(&mut self) -> Option<TopLevel> {
        let mut is_async = false;
        if matches!(self.peek_kind(), TokenKind::Async) {
            self.advance();
            is_async = true;
        }

        match self.peek_kind() {
            TokenKind::Entity => Some(self.parse_entity()),
            TokenKind::Fn => Some(self.parse_fn(is_async)),
            TokenKind::Data => Some(self.parse_data()),
            TokenKind::Kind => Some(self.parse_kind()),
            TokenKind::Import => Some(self.parse_import()),
            TokenKind::Extern => Some(self.parse_extern()),
            TokenKind::Test => Some(self.parse_test(is_async)),
            TokenKind::Mock => Some(self.parse_mock()),
            _ => {
                self.error_at_peek("Expected top-level item".to_string());
                None
            }
        }
    }

    fn parse_entity(&mut self) -> TopLevel {
        self.advance();
        let name = self.parse_identifier();
        self.expect_open_block();
        let body = self.parse_entity_body();
        self.consume_close();
        TopLevel::Entity { name, body }
    }

    fn parse_entity_body(&mut self) -> Vec<EntityItem> {
        let mut body = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
            let start = self.pos;
            if let Some(item) = self.parse_entity_item() {
                body.push(item);
            }
            if self.pos == start {
                self.advance();
            }
        }
        body
    }

    fn parse_entity_item(&mut self) -> Option<EntityItem> {
        let mut is_async = false;
        let mut is_shared = false;

        loop {
            match self.peek_kind() {
                TokenKind::Async => {
                    self.advance();
                    is_async = true;
                }
                TokenKind::Shared => {
                    self.advance();
                    is_shared = true;
                }
                _ => break,
            }
        }

        match self.peek_kind() {
            TokenKind::Has => {
                self.advance();
                let name = self.parse_identifier();
                Some(EntityItem::Has(name))
            }
            TokenKind::When => {
                self.advance();
                let event = self.parse_identifier();
                let params = if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.parse_params()
                } else {
                    Vec::new()
                };
                self.expect_open_block();
                let body = self.parse_stmts();
                self.consume_close();
                Some(EntityItem::When {
                    event,
                    params,
                    body,
                    is_async,
                })
            }
            TokenKind::Identifier(_) => {
                let name = self.parse_identifier();
                if matches!(self.peek_kind(), TokenKind::ColonColon) {
                    self.advance();
                    let (ty, value) = self.parse_typed_decl();
                    Some(EntityItem::Property {
                        name,
                        ty,
                        value,
                        is_shared,
                    })
                } else if matches!(self.peek_kind(), TokenKind::LParen) {
                    let params = self.parse_params();
                    let ret = if matches!(self.peek_kind(), TokenKind::RightArrow) {
                        self.advance();
                        Some(self.parse_type())
                    } else {
                        None
                    };
                    self.expect_open_block();
                    let body = self.parse_stmts();
                    self.consume_close();
                    Some(EntityItem::Method {
                        name,
                        params,
                        ret,
                        body,
                        is_async,
                    })
                } else {
                    self.error_at_peek("Expected '::' or '(' after entity member name".to_string());
                    None
                }
            }
            _ => {
                self.error_at_peek("Expected entity body item".to_string());
                None
            }
        }
    }

    fn parse_typed_decl(&mut self) -> (TypeExpr, Option<Expr>) {
        if self.looks_like_type() {
            let ty = self.parse_type();
            let value = if matches!(self.peek_kind(), TokenKind::Eq) {
                self.advance();
                Some(self.parse_expr())
            } else {
                None
            };
            (ty, value)
        } else {
            let value = self.parse_expr();
            (
                TypeExpr::Named("<inferred>".to_string()),
                Some(value),
            )
        }
    }

    fn looks_like_type(&self) -> bool {
        match self.peek_kind() {
            TokenKind::Maybe | TokenKind::Result => true,
            TokenKind::GameNativeType(_) => true,
            TokenKind::Identifier(s) => {
                matches!(
                    s.as_str(),
                    "int" | "float" | "bool" | "string"
                    | "int8" | "int16" | "int32" | "int64"
                    | "uint8" | "uint16" | "uint32" | "uint64"
                    | "float32" | "float64"
                    | "list" | "map" | "array" | "channel" | "set"
                    | "nothing" | "Entity" | "Vector2" | "Vector3"
                    | "Vector4" | "Quat" | "Color" | "Transform"
                    | "Rect" | "Ray"
                ) || s.chars().next().map_or(false, |c| c.is_uppercase())
            }
            _ => false,
        }
    }

    fn parse_fn(&mut self, is_async: bool) -> TopLevel {
        self.advance();
        let name = self.parse_identifier();
        if matches!(self.peek_kind(), TokenKind::LBracket) {
            self.advance();
            while !matches!(self.peek_kind(), TokenKind::RBracket | TokenKind::EOF) {
                self.advance();
            }
            if matches!(self.peek_kind(), TokenKind::RBracket) {
                self.advance();
            }
        }
        let params = self.parse_params();
        let ret = if matches!(self.peek_kind(), TokenKind::RightArrow) {
            self.advance();
            Some(self.parse_type())
        } else {
            None
        };
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        TopLevel::Fn {
            name,
            params,
            ret,
            body,
            is_async,
        }
    }

    fn parse_data(&mut self) -> TopLevel {
        self.advance();
        let name = self.parse_identifier();
        if matches!(self.peek_kind(), TokenKind::LBracket) {
            self.advance();
            while !matches!(self.peek_kind(), TokenKind::RBracket | TokenKind::EOF) {
                self.advance();
            }
            if matches!(self.peek_kind(), TokenKind::RBracket) {
                self.advance();
            }
        }
        self.expect_open_block();
        let mut fields = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
            let start = self.pos;
            if let TokenKind::Identifier(_) = self.peek_kind() {
                let field_name = self.parse_identifier();
                if matches!(self.peek_kind(), TokenKind::ColonColon) {
                    self.advance();
                    let (ty, value) = self.parse_typed_decl();
                    fields.push(Field {
                        name: field_name,
                        ty,
                        value,
                    });
                } else {
                    self.error_at_peek("Expected '::' in data field".to_string());
                }
            } else {
                self.error_at_peek("Expected field name in data".to_string());
            }
            if self.pos == start {
                self.advance();
            }
        }
        self.consume_close();
        TopLevel::Data { name, fields }
    }

    fn parse_kind(&mut self) -> TopLevel {
        self.advance();
        let name = self.parse_identifier();
        self.expect_open_block();
        let mut variants = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
            let start = self.pos;
            if let TokenKind::Identifier(_) = self.peek_kind() {
                let n = self.parse_identifier();
                variants.push(KindVariant { name: n });
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    let mut depth: i32 = 0;
                    loop {
                        match self.peek_kind() {
                            TokenKind::LParen => {
                                depth += 1;
                                self.advance();
                            }
                            TokenKind::RParen => {
                                depth -= 1;
                                self.advance();
                                if depth <= 0 {
                                    break;
                                }
                            }
                            TokenKind::EOF => break,
                            _ => {
                                self.advance();
                            }
                        }
                    }
                }
            } else {
                self.error_at_peek("Expected kind variant".to_string());
            }
            if self.pos == start {
                self.advance();
            }
        }
        self.consume_close();
        TopLevel::Kind { name, variants }
    }

    fn parse_import(&mut self) -> TopLevel {
        self.advance();
        let mut path = self.parse_identifier();
        while matches!(self.peek_kind(), TokenKind::Dot) {
            self.advance();
            let next = self.parse_identifier();
            path.push('.');
            path.push_str(&next);
        }
        TopLevel::Import(path)
    }

    fn parse_extern(&mut self) -> TopLevel {
        self.advance();
        let lang = self.parse_string_literal();
        let name = self.parse_identifier();
        self.expect_open_block();
        let mut items = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
            let start = self.pos;
            if let TokenKind::Identifier(_) = self.peek_kind() {
                let item_name = self.parse_identifier();
                let params = if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.parse_params()
                } else {
                    Vec::new()
                };
                let ret = if matches!(self.peek_kind(), TokenKind::RightArrow) {
                    self.advance();
                    Some(self.parse_type())
                } else {
                    None
                };
                items.push(ExternItem {
                    name: item_name,
                    params,
                    ret,
                });
            } else {
                self.error_at_peek("Expected extern item".to_string());
            }
            if self.pos == start {
                self.advance();
            }
        }
        self.consume_close();
        TopLevel::Extern { lang, name, items }
    }

    fn parse_string_literal(&mut self) -> String {
        let mut content = String::new();
        if matches!(self.peek_kind(), TokenKind::StringStart) {
            self.advance();
            loop {
                let kind = self.peek_kind().clone();
                match kind {
                    TokenKind::StringContent(s) => {
                        content.push_str(&s);
                        self.advance();
                    }
                    TokenKind::StringEnd => {
                        self.advance();
                        break;
                    }
                    TokenKind::EOF => break,
                    _ => {
                        self.advance();
                    }
                }
            }
        } else {
            self.error_at_peek("Expected string literal".to_string());
        }
        content
    }

    fn parse_test(&mut self, is_async: bool) -> TopLevel {
        self.advance();
        let description = self.parse_string_literal();
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        TopLevel::Test {
            description,
            body,
            is_async,
        }
    }

    fn parse_mock(&mut self) -> TopLevel {
        self.advance();
        let target = self.parse_identifier();
        self.expect_open_block();
        let mut body = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
            let start = self.pos;
            if matches!(self.peek_kind(), TokenKind::When) {
                self.advance();
                let event = self.parse_identifier();
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    let _ = self.parse_params();
                }
                self.expect_open_block();
                let stmts = self.parse_stmts();
                self.consume_close();
                body.push(MockItem { event, body: stmts });
            } else {
                self.error_at_peek("Expected 'when' in mock body".to_string());
            }
            if self.pos == start {
                self.advance();
            }
        }
        self.consume_close();
        TopLevel::Mock { target, body }
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::LParen) {
            return params;
        }
        self.advance();
        while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::EOF) {
            let start = self.pos;
            if let TokenKind::Identifier(_) = self.peek_kind() {
                let name = self.parse_identifier();
                let ty = if matches!(self.peek_kind(), TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else if self.looks_like_type() {
                    self.parse_type()
                } else {
                    TypeExpr::Named("<inferred>".to_string())
                };
                params.push(Param { name, ty });
            } else {
                self.error_at_peek("Expected parameter name".to_string());
            }
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
            if self.pos == start {
                self.advance();
            }
        }
        if matches!(self.peek_kind(), TokenKind::RParen) {
            self.advance();
        } else {
            self.error_at_peek("Expected ')'".to_string());
        }
        params
    }

    fn parse_stmts(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
            let start = self.pos;
            stmts.push(self.parse_stmt());
            if self.pos == start {
                self.advance();
            }
        }
        stmts
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.peek_kind() {
            TokenKind::Fail => {
                self.advance();
                let expr = self.parse_expr();
                Stmt::Fail(expr)
            }
            TokenKind::Succeed => {
                self.advance();
                let expr = self.parse_expr();
                Stmt::Succeed(expr)
            }
            TokenKind::Result => {
                self.advance();
                let expr = self.parse_expr();
                Stmt::Result(expr)
            }
            TokenKind::Event => {
                self.advance();
                let expr = self.parse_expr();
                Stmt::Event(expr)
            }
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::Loop => self.parse_loop_stmt(),
            TokenKind::Every => self.parse_every_stmt(),
            TokenKind::Match => {
                self.advance();
                let subject = self.parse_expr();
                self.expect_open_block();
                let mut arms = Vec::new();
                while !matches!(self.peek_kind(), TokenKind::CloseBlock | TokenKind::EOF) {
                    let start = self.pos;
                    arms.push(self.parse_match_arm());
                    if self.pos == start {
                        self.advance();
                    }
                }
                self.consume_close();
                Stmt::Match { subject, arms }
            }
            TokenKind::Await => {
                self.advance();
                let expr = self.parse_expr();
                Stmt::Await(expr)
            }
            TokenKind::Task => self.parse_task_stmt(),
            TokenKind::Stop => {
                self.advance();
                Stmt::Stop
            }
            TokenKind::Skip => {
                self.advance();
                Stmt::Skip
            }
            _ => {
                if matches!(self.peek_kind(), TokenKind::Identifier(_))
                    && matches!(self.peek_offset(1).kind, TokenKind::ColonColon)
                {
                    return self.parse_var_decl();
                }
                let lhs = self.parse_expr();
                let op = match self.peek_kind() {
                    TokenKind::Eq => Some(AssignOp::Eq),
                    TokenKind::PlusEq => Some(AssignOp::PlusEq),
                    TokenKind::MinusEq => Some(AssignOp::MinusEq),
                    TokenKind::StarEq => Some(AssignOp::StarEq),
                    TokenKind::SlashEq => Some(AssignOp::SlashEq),
                    TokenKind::WrapAddEq => Some(AssignOp::WrapAddEq),
                    TokenKind::WrapSubEq => Some(AssignOp::WrapSubEq),
                    TokenKind::WrapMulEq => Some(AssignOp::WrapMulEq),
                    TokenKind::SatAddEq => Some(AssignOp::SatAddEq),
                    TokenKind::SatSubEq => Some(AssignOp::SatSubEq),
                    TokenKind::SatMulEq => Some(AssignOp::SatMulEq),
                    TokenKind::PanicAddEq => Some(AssignOp::PanicAddEq),
                    TokenKind::PanicSubEq => Some(AssignOp::PanicSubEq),
                    TokenKind::PanicMulEq => Some(AssignOp::PanicMulEq),
                    _ => None,
                };
                if let Some(op) = op {
                    self.advance();
                    let rhs = self.parse_expr();
                    Stmt::Assign {
                        target: lhs,
                        op,
                        value: rhs,
                    }
                } else {
                    Stmt::ExprStmt(lhs)
                }
            }
        }
    }

    fn parse_if_stmt(&mut self) -> Stmt {
        self.advance();
        let condition = self.parse_expr();
        self.expect_open_block();
        let then = self.parse_stmts();
        self.consume_close();
        let else_ = if matches!(self.peek_kind(), TokenKind::Else) {
            self.advance();
            if matches!(self.peek_kind(), TokenKind::If) {
                let nested = self.parse_if_stmt();
                Some(vec![nested])
            } else {
                self.expect_open_block();
                let body = self.parse_stmts();
                self.consume_close();
                Some(body)
            }
        } else {
            None
        };
        Stmt::If {
            condition,
            then,
            else_,
        }
    }

    fn parse_loop_stmt(&mut self) -> Stmt {
        self.advance(); // consume `loop`

        if matches!(self.peek_kind(), TokenKind::While) {
            return self.parse_loop_while();
        }

        if matches!(self.peek_kind(), TokenKind::OpenBlock) {
            self.expect_open_block();
            let body = self.parse_stmts();
            self.consume_close();
            return Stmt::Loop {
                kind: LoopKind::While,
                var_name: None,
                from_expr: None,
                to_expr: None,
                iterable: None,
                condition: None,
                body,
            };
        }

        let var_name = self.parse_identifier();
        match self.peek_kind() {
            TokenKind::From => self.parse_loop_range(var_name),
            TokenKind::In => self.parse_loop_iterate(var_name),
            _ => {
                self.error_at_peek(
                    "Expected 'from' or 'in' after loop variable".to_string(),
                );
                while !matches!(self.peek_kind(), TokenKind::OpenBlock | TokenKind::EOF) {
                    self.advance();
                }
                self.expect_open_block();
                let body = self.parse_stmts();
                self.consume_close();
                Stmt::Loop {
                    kind: LoopKind::Range,
                    var_name: Some(var_name),
                    from_expr: None,
                    to_expr: None,
                    iterable: None,
                    condition: None,
                    body,
                }
            }
        }
    }

    fn parse_loop_range(&mut self, var_name: String) -> Stmt {
        self.advance(); // consume `from`
        let from_expr = self.parse_expr();
        if matches!(self.peek_kind(), TokenKind::To) {
            self.advance();
        } else {
            self.error_at_peek("Expected 'to' in range loop".to_string());
        }
        let to_expr = self.parse_expr();
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        Stmt::Loop {
            kind: LoopKind::Range,
            var_name: Some(var_name),
            from_expr: Some(from_expr),
            to_expr: Some(to_expr),
            iterable: None,
            condition: None,
            body,
        }
    }

    fn parse_loop_iterate(&mut self, var_name: String) -> Stmt {
        self.advance(); // consume `in`
        let iterable = self.parse_expr();
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        Stmt::Loop {
            kind: LoopKind::Iterate,
            var_name: Some(var_name),
            from_expr: None,
            to_expr: None,
            iterable: Some(iterable),
            condition: None,
            body,
        }
    }

    fn parse_loop_while(&mut self) -> Stmt {
        self.advance(); // consume `while`
        let condition = self.parse_expr();
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        Stmt::Loop {
            kind: LoopKind::While,
            var_name: None,
            from_expr: None,
            to_expr: None,
            iterable: None,
            condition: Some(condition),
            body,
        }
    }

    fn parse_every_stmt(&mut self) -> Stmt {
        self.advance();
        let interval = self.parse_expr();
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        Stmt::Every { interval, body }
    }

    fn parse_task_stmt(&mut self) -> Stmt {
        self.advance();
        if matches!(self.peek_kind(), TokenKind::Identifier(_)) {
            self.advance();
        }
        while !matches!(self.peek_kind(), TokenKind::OpenBlock | TokenKind::EOF) {
            self.advance();
        }
        self.expect_open_block();
        let body = self.parse_stmts();
        self.consume_close();
        Stmt::Task { body }
    }

    fn parse_var_decl(&mut self) -> Stmt {
        let name = self.parse_identifier();
        self.advance();
        if self.looks_like_type() {
            let ty = self.parse_type();
            let value = if matches!(self.peek_kind(), TokenKind::Eq) {
                self.advance();
                Some(self.parse_expr())
            } else {
                None
            };
            Stmt::VarDecl {
                name,
                ty: Some(ty),
                value,
            }
        } else {
            let value = self.parse_expr();
            Stmt::VarDecl {
                name,
                ty: None,
                value: Some(value),
            }
        }
    }

    fn parse_match_arm(&mut self) -> MatchArm {
        let pattern = self.parse_match_pattern();
        let guard = None;
        if matches!(self.peek_kind(), TokenKind::RightArrow) {
            self.advance();
            let stmt = self.parse_stmt();
            MatchArm {
                pattern,
                guard,
                body: vec![stmt],
            }
        } else if matches!(self.peek_kind(), TokenKind::OpenBlock) {
            self.advance();
            let body = self.parse_stmts();
            self.consume_close();
            MatchArm {
                pattern,
                guard,
                body,
            }
        } else {
            self.error_at_peek("Expected '->' or '>>' in match arm".to_string());
            MatchArm {
                pattern,
                guard,
                body: Vec::new(),
            }
        }
    }

    fn parse_match_pattern(&mut self) -> MatchPattern {
        if let TokenKind::Identifier(s) = self.peek_kind() {
            if s == "_" {
                self.advance();
                return MatchPattern::Wildcard;
            }
        }
        let e = self.parse_expr();
        if let Expr::Identifier(name) = &e {
            return MatchPattern::Identifier(name.clone());
        }
        MatchPattern::Literal(e)
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_expr_prec(0)
    }

    fn parse_expr_prec(&mut self, min_prec: u8) -> Expr {
        let mut left = self.parse_unary();
        loop {
            let (op, prec) = match self.peek_kind() {
                TokenKind::EqEq => (BinOp::Eq, 2),
                TokenKind::NotEq => (BinOp::NotEq, 2),
                TokenKind::Lt => (BinOp::Lt, 2),
                TokenKind::Gt => (BinOp::Gt, 2),
                TokenKind::LtEq => (BinOp::LtEq, 2),
                TokenKind::GtEq => (BinOp::GtEq, 2),
                TokenKind::Plus => (BinOp::Add, 3),
                TokenKind::Minus => (BinOp::Sub, 3),
                TokenKind::WrapAdd => (BinOp::WrapAdd, 3),
                TokenKind::WrapSub => (BinOp::WrapSub, 3),
                TokenKind::SatAdd => (BinOp::SatAdd, 3),
                TokenKind::SatSub => (BinOp::SatSub, 3),
                TokenKind::PanicAdd => (BinOp::PanicAdd, 3),
                TokenKind::PanicSub => (BinOp::PanicSub, 3),
                TokenKind::Star => (BinOp::Mul, 4),
                TokenKind::Slash => (BinOp::Div, 4),
                TokenKind::WrapMul => (BinOp::WrapMul, 4),
                TokenKind::SatMul => (BinOp::SatMul, 4),
                TokenKind::PanicMul => (BinOp::PanicMul, 4),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.advance();
            let right = self.parse_expr_prec(prec + 1);
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_unary(&mut self) -> Expr {
        match self.peek_kind() {
            TokenKind::Minus => {
                self.advance();
                let e = self.parse_unary();
                Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    expr: Box::new(e),
                }
            }
            TokenKind::Bang => {
                self.advance();
                let e = self.parse_unary();
                Expr::UnaryOp {
                    op: UnaryOp::Not,
                    expr: Box::new(e),
                }
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut left = self.parse_primary();
        loop {
            match self.peek_kind() {
                TokenKind::Dot => {
                    self.advance();
                    let field = self.parse_identifier();
                    if field == "as" && matches!(self.peek_kind(), TokenKind::LParen) {
                        self.advance();
                        let ty = self.parse_type();
                        if matches!(self.peek_kind(), TokenKind::RParen) {
                            self.advance();
                        }
                        left = Expr::As {
                            value: Box::new(left),
                            ty,
                        };
                    } else {
                        left = Expr::FieldAccess {
                            object: Box::new(left),
                            field,
                        };
                    }
                }
                TokenKind::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::EOF) {
                        let start = self.pos;
                        let value = self.parse_expr();
                        args.push(Arg { label: None, value });
                        if matches!(self.peek_kind(), TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                        if self.pos == start {
                            self.advance();
                        }
                    }
                    if matches!(self.peek_kind(), TokenKind::RParen) {
                        self.advance();
                    }
                    left = Expr::Call {
                        callee: Box::new(left),
                        args,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr();
                    if matches!(self.peek_kind(), TokenKind::RBracket) {
                        self.advance();
                    }
                    left = Expr::Index {
                        object: Box::new(left),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_primary(&mut self) -> Expr {
        let kind = self.peek_kind().clone();
        match kind {
            TokenKind::IntLiteral(n) => {
                self.advance();
                Expr::IntLiteral(n)
            }
            TokenKind::FloatLiteral(f) => {
                self.advance();
                Expr::FloatLiteral(f)
            }
            TokenKind::BoolLiteral(b) => {
                self.advance();
                Expr::BoolLiteral(b)
            }
            TokenKind::Identifier(s) => {
                self.advance();
                Expr::Identifier(s)
            }
            TokenKind::GameNativeType(s) => {
                self.advance();
                Expr::Identifier(s)
            }
            TokenKind::StringStart => {
                self.advance();
                let mut parts = Vec::new();
                loop {
                    let k = self.peek_kind().clone();
                    match k {
                        TokenKind::StringContent(s) => {
                            parts.push(StringPart::Literal(s));
                            self.advance();
                        }
                        TokenKind::InterpolationExpr(s) => {
                            parts.push(StringPart::Interpolation(Expr::Identifier(s)));
                            self.advance();
                        }
                        TokenKind::StringEnd => {
                            self.advance();
                            break;
                        }
                        TokenKind::EOF => break,
                        _ => break,
                    }
                }
                Expr::StringLiteral(parts)
            }
            TokenKind::LParen => {
                self.advance();
                let e = self.parse_expr();
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    self.advance();
                }
                e
            }
            TokenKind::LBracket => {
                self.advance();
                if matches!(self.peek_kind(), TokenKind::RBracket) {
                    self.advance();
                    return Expr::ListLiteral(Vec::new());
                }
                let first = self.parse_expr();
                if matches!(self.peek_kind(), TokenKind::RightArrow) {
                    self.advance();
                    let v = self.parse_expr();
                    let mut pairs = vec![(first, v)];
                    while matches!(self.peek_kind(), TokenKind::Comma) {
                        self.advance();
                        if matches!(self.peek_kind(), TokenKind::RBracket) {
                            break;
                        }
                        let k = self.parse_expr();
                        if matches!(self.peek_kind(), TokenKind::RightArrow) {
                            self.advance();
                        }
                        let val = self.parse_expr();
                        pairs.push((k, val));
                    }
                    if matches!(self.peek_kind(), TokenKind::RBracket) {
                        self.advance();
                    }
                    Expr::MapLiteral(pairs)
                } else {
                    let mut items = vec![first];
                    while matches!(self.peek_kind(), TokenKind::Comma) {
                        self.advance();
                        if matches!(self.peek_kind(), TokenKind::RBracket) {
                            break;
                        }
                        items.push(self.parse_expr());
                    }
                    if matches!(self.peek_kind(), TokenKind::RBracket) {
                        self.advance();
                    }
                    Expr::ListLiteral(items)
                }
            }
            TokenKind::RightArrow => {
                self.advance();
                if matches!(self.peek_kind(), TokenKind::Await) {
                    self.advance();
                    let e = self.parse_expr();
                    Expr::Arrow(Box::new(Expr::Async(Box::new(e))))
                } else {
                    let e = self.parse_expr();
                    Expr::Arrow(Box::new(e))
                }
            }
            TokenKind::Await => {
                self.advance();
                let e = self.parse_expr();
                Expr::Async(Box::new(e))
            }
            _ => {
                self.error_at_peek(format!("Unexpected token in expression: {:?}", kind));
                self.advance();
                Expr::Identifier("<error>".to_string())
            }
        }
    }

    fn parse_type(&mut self) -> TypeExpr {
        let kind = self.peek_kind().clone();
        match kind {
            TokenKind::Maybe => {
                self.advance();
                TypeExpr::Maybe(Box::new(self.parse_type()))
            }
            TokenKind::Result => {
                self.advance();
                if matches!(self.peek_kind(), TokenKind::LBracket) {
                    self.advance();
                    let inner = self.parse_type();
                    if matches!(self.peek_kind(), TokenKind::RBracket) {
                        self.advance();
                    }
                    TypeExpr::Result(Box::new(inner))
                } else {
                    TypeExpr::Named("result".to_string())
                }
            }
            TokenKind::GameNativeType(name) => {
                self.advance();
                TypeExpr::GameNative(name)
            }
            TokenKind::Identifier(name) => {
                self.advance();
                if matches!(self.peek_kind(), TokenKind::LBracket) {
                    self.advance();
                    let mut args = vec![self.parse_type()];
                    while matches!(self.peek_kind(), TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_type());
                    }
                    if matches!(self.peek_kind(), TokenKind::RBracket) {
                        self.advance();
                    }
                    return match name.as_str() {
                        "list" if args.len() == 1 => {
                            let mut it = args.into_iter();
                            TypeExpr::List(Box::new(it.next().unwrap()))
                        }
                        "channel" if args.len() == 1 => {
                            let mut it = args.into_iter();
                            TypeExpr::Channel(Box::new(it.next().unwrap()))
                        }
                        _ => TypeExpr::Generic { base: name, args },
                    };
                }
                TypeExpr::Named(name)
            }
            _ => {
                self.error_at_peek(format!("Expected type, found {:?}", self.peek_kind()));
                TypeExpr::Named("<error>".to_string())
            }
        }
    }
}

pub fn parse(tokens: Vec<Token>) -> (Program, Vec<ParseError>) {
    let mut p = Parser::new(tokens);
    let program = p.parse_program();
    (program, p.errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_source(src: &str) -> (Program, Vec<ParseError>) {
        let (tokens, _) = lex(src);
        parse(tokens)
    }

    #[test]
    fn test_empty_program() {
        let (program, errors) = parse_source("");
        assert!(errors.is_empty());
        assert!(program.items.is_empty());
    }

    #[test]
    fn test_parse_entity() {
        let src = "entity Player >>\n<< Player";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Entity { .. }));
    }

    #[test]
    fn test_parse_fn() {
        let src = "fn add(x: int, y: int) -> int >>\n<< add";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Fn { .. }));
    }

    #[test]
    fn test_parse_async_fn() {
        let src = "async fn load() >>\n<< load";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        if let TopLevel::Fn { is_async, .. } = &program.items[0] {
            assert!(is_async);
        }
    }

    #[test]
    fn test_parse_data() {
        let src = "data Point >>\n    x :: float\n    y :: float\n<< Point";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Data { .. }));
    }

    #[test]
    fn test_parse_kind() {
        let src = "kind Direction >>\n    north\n    south\n<< Direction";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Kind { .. }));
    }

    #[test]
    fn test_parse_import() {
        let src = "import skev.math";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Import(_)));
    }

    #[test]
    fn test_parse_when_handler() {
        let src = "entity Player >>\n    when update(delta: float) >>\n    << update\n<< Player";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_if() {
        let src = "fn f() >>\n    if x == 1 >>\n        fail x\n    << x == 1\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_match() {
        let src = "fn f() >>\n    match x >>\n        1 -> succeed x\n        _ -> fail x\n    << x\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_maybe_type() {
        let src = "fn f() -> maybe int >>\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        if let TopLevel::Fn { ret, .. } = &program.items[0] {
            assert!(matches!(ret, Some(TypeExpr::Maybe(_))));
        }
    }

    #[test]
    fn test_parse_result_type() {
        let src = "fn f() -> result[int] >>\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_list_literal() {
        let src = "fn f() >>\n    x :: list[int] = [1, 2, 3]\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_extern() {
        let src = "extern \"C\" Raylib >>\n    init_window(width: int, height: int)\n<< Raylib";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Extern { .. }));
    }

    #[test]
    fn test_parse_wrapping_binop() {
        let src = "fn f(x: int, y: int) -> int >>\n    result x +% y\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Fn { .. }));
    }

    #[test]
    fn test_parse_saturating_binop() {
        let src = "fn f(x: int, y: int) -> int >>\n    result x +| y\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_panic_binop() {
        let src = "fn f(x: int, y: int) -> int >>\n    result x +! y\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_wrapping_assign() {
        let src = "fn f() >>\n    x :: int = 0\n    x +%= 1\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_saturating_assign() {
        let src = "fn f() >>\n    x :: int = 0\n    x +|= 1\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_panic_assign() {
        let src = "fn f() >>\n    x :: int = 0\n    x +!= 1\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_overflow_precedence() {
        // x +% y *% z should parse as x +% (y *% z)
        // same as x + y * z — multiplicative binds tighter
        let src = "fn f(x: int, y: int, z: int) -> int >>\n    result x +% y *% z\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_property_game_native_type() {
        let src = "entity Player >>\n    pos :: Vector3!\n<< Player";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Entity { .. }));
    }

    #[test]
    fn test_parse_param_game_native_type() {
        let src = "fn move_to(target: Vector3!) >>\n<< move_to";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        if let TopLevel::Fn { params, .. } = &program.items[0] {
            assert!(matches!(&params[0].ty, TypeExpr::GameNative(s) if s == "Vector3!"));
        }
    }

    #[test]
    fn test_parse_return_game_native_type() {
        let src = "fn origin() -> Vector3! >>\n<< origin";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        if let TopLevel::Fn { ret, .. } = &program.items[0] {
            assert!(matches!(ret, Some(TypeExpr::GameNative(s)) if s == "Vector3!"));
        }
    }

    #[test]
    fn test_parse_data_field_game_native_type() {
        let src = "data Particle >>\n    point :: Vector3!\n<< Particle";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
        assert!(matches!(program.items[0], TopLevel::Data { .. }));
    }

    #[test]
    fn test_parse_game_native_in_expression() {
        let src = "fn f() >>\n    Vector3!.zero\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty());
    }

    fn fn_body(p: &Program) -> &Vec<Stmt> {
        match &p.items[0] {
            TopLevel::Fn { body, .. } => body,
            _ => panic!("expected fn"),
        }
    }

    #[test]
    fn test_parse_loop_range() {
        let src = "fn f() >>\n    loop i from 0 to 10 >>\n    << i\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        match &body[0] {
            Stmt::Loop { kind, var_name, .. } => {
                assert_eq!(*kind, LoopKind::Range);
                assert_eq!(var_name.as_deref(), Some("i"));
            }
            other => panic!("expected Loop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_loop_range_expr() {
        let src = "fn f() >>\n    loop i from start to end >>\n    << i\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        match &body[0] {
            Stmt::Loop {
                kind,
                from_expr,
                to_expr,
                ..
            } => {
                assert_eq!(*kind, LoopKind::Range);
                assert!(matches!(from_expr, Some(Expr::Identifier(s)) if s == "start"));
                assert!(matches!(to_expr, Some(Expr::Identifier(s)) if s == "end"));
            }
            other => panic!("expected Loop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_loop_iterate() {
        let src = "fn f() >>\n    loop item in items >>\n    << item\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        match &body[0] {
            Stmt::Loop {
                kind,
                var_name,
                iterable,
                ..
            } => {
                assert_eq!(*kind, LoopKind::Iterate);
                assert_eq!(var_name.as_deref(), Some("item"));
                assert!(matches!(iterable, Some(Expr::Identifier(s)) if s == "items"));
            }
            other => panic!("expected Loop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_loop_while() {
        let src = "fn f() >>\n    loop while alive >>\n    << while alive\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        match &body[0] {
            Stmt::Loop {
                kind, condition, ..
            } => {
                assert_eq!(*kind, LoopKind::While);
                assert!(matches!(condition, Some(Expr::Identifier(s)) if s == "alive"));
            }
            other => panic!("expected Loop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_loop_with_body() {
        let src = "fn f() >>\n    loop i from 0 to 5 >>\n        log(i)\n    << i\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        match &body[0] {
            Stmt::Loop { body: loop_body, .. } => {
                assert_eq!(loop_body.len(), 1);
            }
            other => panic!("expected Loop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_loop_nested() {
        let src = "fn f() >>\n    loop i from 0 to 3 >>\n        loop j from 0 to 3 >>\n        << j\n    << i\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        match &body[0] {
            Stmt::Loop {
                body: outer,
                var_name,
                ..
            } => {
                assert_eq!(var_name.as_deref(), Some("i"));
                assert_eq!(outer.len(), 1);
                assert!(
                    matches!(&outer[0], Stmt::Loop { var_name: Some(n), .. } if n == "j")
                );
            }
            other => panic!("expected Loop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_stop() {
        let src = "fn f() >>\n    stop\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        assert!(matches!(body[0], Stmt::Stop));
    }

    #[test]
    fn test_parse_skip() {
        let src = "fn f() >>\n    skip\n<< f";
        let (program, errors) = parse_source(src);
        assert!(errors.is_empty(), "{:?}", errors);
        let body = fn_body(&program);
        assert!(matches!(body[0], Stmt::Skip));
    }
}
