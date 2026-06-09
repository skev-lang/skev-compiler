use std::collections::HashMap;

use crate::parser::{
    BinOp, EntityItem, Expr, Field, MatchArm, MatchPattern, Program, Stmt, StringPart, TopLevel,
    TypeExpr, UnaryOp,
};
use crate::types::game_native::{self, BUILTIN_GAME_NATIVE_TYPES};

#[derive(Debug, Clone, PartialEq)]
pub enum SkevType {
    Int,
    Float,
    Bool,
    String,
    Int32,
    Int64,
    Float32,
    Float64,
    Vector3,
    Color,
    Texture2D,
    Sound,
    Music,
    GameNative(String), // catch-all for game-native types not in the known list

    List(Box<SkevType>),
    Array(Box<SkevType>, usize),
    Map(Box<SkevType>, Box<SkevType>),
    Maybe(Box<SkevType>),
    Result(Box<SkevType>),
    Channel(Box<SkevType>),
    Entity(String),
    Data(String),
    Kind(String),
    Fn {
        params: Vec<SkevType>,
        ret: Box<SkevType>,
    },
    Nothing,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Variable,
    Function,
    Entity,
    Parameter,
    Property,
    Kind,
    Data,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub ty: SkevType,
    pub kind: SymbolKind,
    pub mutable: bool,
}

pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, symbol: Symbol) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(symbol.name.clone(), symbol);
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(s) = scope.get(name) {
                return Some(s);
            }
        }
        None
    }

    pub fn lookup_global(&self, name: &str) -> Option<&Symbol> {
        self.scopes.first().and_then(|s| s.get(name))
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

struct TypeChecker {
    errors: Vec<TypeError>,
    symbols: SymbolTable,
}

impl TypeChecker {
    fn new() -> Self {
        TypeChecker {
            errors: Vec::new(),
            symbols: SymbolTable::new(),
        }
    }

    fn error(&mut self, msg: String) {
        self.errors.push(TypeError {
            message: msg,
            line: 0,
            col: 0,
        });
    }

    fn convert_type(&mut self, te: &TypeExpr) -> SkevType {
        match te {
            TypeExpr::Named(s) => match s.as_str() {
                "int" => SkevType::Int,
                "float" => SkevType::Float,
                "bool" => SkevType::Bool,
                "string" => SkevType::String,
                "int32" => SkevType::Int32,
                "int64" => SkevType::Int64,
                "float32" => SkevType::Float32,
                "float64" => SkevType::Float64,
                "nothing" => SkevType::Nothing,
                "Texture2D" => SkevType::Texture2D,
                "Sound" => SkevType::Sound,
                "Music" => SkevType::Music,
                "<inferred>" | "<error>" => SkevType::Unknown,
                other => {
                    if let Some(sym) = self.symbols.lookup(other) {
                        match sym.kind {
                            SymbolKind::Entity | SymbolKind::Data | SymbolKind::Kind => {
                                sym.ty.clone()
                            }
                            _ => SkevType::Unknown,
                        }
                    } else {
                        SkevType::Unknown
                    }
                }
            },
            TypeExpr::Generic { base, args } => match base.as_str() {
                "list" if args.len() == 1 => SkevType::List(Box::new(self.convert_type(&args[0]))),
                "channel" if args.len() == 1 => {
                    SkevType::Channel(Box::new(self.convert_type(&args[0])))
                }
                "map" if args.len() == 2 => SkevType::Map(
                    Box::new(self.convert_type(&args[0])),
                    Box::new(self.convert_type(&args[1])),
                ),
                "array" if !args.is_empty() => {
                    SkevType::Array(Box::new(self.convert_type(&args[0])), 0)
                }
                "result" if args.len() == 1 => {
                    SkevType::Result(Box::new(self.convert_type(&args[0])))
                }
                "maybe" if args.len() == 1 => {
                    SkevType::Maybe(Box::new(self.convert_type(&args[0])))
                }
                _ => SkevType::Unknown,
            },
            TypeExpr::Maybe(t) => SkevType::Maybe(Box::new(self.convert_type(t))),
            TypeExpr::Result(t) => SkevType::Result(Box::new(self.convert_type(t))),
            TypeExpr::List(t) => SkevType::List(Box::new(self.convert_type(t))),
            TypeExpr::Channel(t) => SkevType::Channel(Box::new(self.convert_type(t))),
            TypeExpr::GameNative(s) => {
                if !game_native::is_known(s) {
                    let stripped = s.trim_end_matches('!');
                    self.errors.push(TypeError {
                        message: format!(
                            "unknown game-native type '{s}'.\n\
                             If '{stripped}' is an entity, use '{stripped}' or \
                             'maybe {stripped}' as the type.\n\
                             Known game-native types: {known}.\n\
                             User-defined game-native types are not yet \
                             supported in v1.0.",
                            s = s,
                            stripped = stripped,
                            known = BUILTIN_GAME_NATIVE_TYPES.join(", "),
                        ),
                        line: 0,
                        col: 0,
                    });
                    return SkevType::Unknown;
                }
                match s.as_str() {
                    "Vector3!" => SkevType::Vector3,
                    "Color!" => SkevType::Color,
                    _ => SkevType::GameNative(s.clone()),
                }
            }
            TypeExpr::Array { ty, size } => SkevType::Array(Box::new(self.convert_type(ty)), *size),
        }
    }

    fn register_top_level(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Entity { name, .. } => {
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: SkevType::Entity(name.clone()),
                        kind: SymbolKind::Entity,
                        mutable: false,
                    });
                }
                TopLevel::Data { name, .. } => {
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: SkevType::Data(name.clone()),
                        kind: SymbolKind::Data,
                        mutable: false,
                    });
                }
                TopLevel::Kind { name, .. } => {
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: SkevType::Kind(name.clone()),
                        kind: SymbolKind::Kind,
                        mutable: false,
                    });
                }
                TopLevel::Fn {
                    name, params, ret, ..
                } => {
                    let param_types: Vec<SkevType> =
                        params.iter().map(|p| self.convert_type(&p.ty)).collect();
                    let ret_type = ret
                        .as_ref()
                        .map(|t| self.convert_type(t))
                        .unwrap_or(SkevType::Nothing);
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: SkevType::Fn {
                            params: param_types,
                            ret: Box::new(ret_type),
                        },
                        kind: SymbolKind::Function,
                        mutable: false,
                    });
                }
                _ => {}
            }
        }
    }

    fn check_top_level(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Entity { body, .. } => {
                    self.symbols.push_scope();
                    for entity_item in body {
                        self.check_entity_item(entity_item);
                    }
                    self.symbols.pop_scope();
                }
                TopLevel::Fn { params, body, .. } => {
                    self.symbols.push_scope();
                    for p in params {
                        let ty = self.convert_type(&p.ty);
                        self.symbols.define(Symbol {
                            name: p.name.clone(),
                            ty,
                            kind: SymbolKind::Parameter,
                            mutable: false,
                        });
                    }
                    for s in body {
                        self.check_stmt(s);
                    }
                    self.symbols.pop_scope();
                }
                TopLevel::Data { fields, .. } => {
                    for f in fields {
                        self.check_field(f);
                    }
                }
                _ => {}
            }
        }
    }

    fn check_entity_item(&mut self, item: &EntityItem) {
        match item {
            EntityItem::Property {
                name, ty, value, ..
            } => {
                let declared = self.convert_type(ty);
                let value_ty = value.as_ref().map(|v| self.type_of_expr(v));
                if let Some(vt) = &value_ty {
                    if declared != SkevType::Unknown
                        && *vt != SkevType::Unknown
                        && declared != *vt
                    {
                        self.error(format!(
                            "Type mismatch in property '{}': expected {:?}, got {:?}",
                            name, declared, vt
                        ));
                    }
                }
                let final_ty = if declared == SkevType::Unknown {
                    value_ty.unwrap_or(SkevType::Unknown)
                } else {
                    declared
                };
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: final_ty,
                    kind: SymbolKind::Property,
                    mutable: true,
                });
            }
            EntityItem::Has(_) => {}
            EntityItem::When { params, body, .. } => {
                self.symbols.push_scope();
                for p in params {
                    let ty = self.convert_type(&p.ty);
                    self.symbols.define(Symbol {
                        name: p.name.clone(),
                        ty,
                        kind: SymbolKind::Parameter,
                        mutable: false,
                    });
                }
                for s in body {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
            }
            EntityItem::Method { params, body, .. } => {
                self.symbols.push_scope();
                for p in params {
                    let ty = self.convert_type(&p.ty);
                    self.symbols.define(Symbol {
                        name: p.name.clone(),
                        ty,
                        kind: SymbolKind::Parameter,
                        mutable: false,
                    });
                }
                for s in body {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
            }
        }
    }

    fn check_field(&mut self, f: &Field) {
        let declared = self.convert_type(&f.ty);
        if let Some(v) = &f.value {
            let value_ty = self.type_of_expr(v);
            if declared != SkevType::Unknown
                && value_ty != SkevType::Unknown
                && declared != value_ty
            {
                self.error(format!(
                    "Type mismatch in field '{}': expected {:?}, got {:?}",
                    f.name, declared, value_ty
                ));
            }
        }
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::VarDecl { name, ty, value } => {
                let declared = ty.as_ref().map(|t| self.convert_type(t));
                let value_ty = value.as_ref().map(|v| self.type_of_expr(v));
                if let (Some(d), Some(v)) = (&declared, &value_ty) {
                    if *d != SkevType::Unknown && *v != SkevType::Unknown && d != v {
                        self.error(format!(
                            "Type mismatch in declaration of '{}': expected {:?}, got {:?}",
                            name, d, v
                        ));
                    }
                }
                let final_ty = declared.or(value_ty).unwrap_or(SkevType::Unknown);
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: final_ty,
                    kind: SymbolKind::Variable,
                    mutable: true,
                });
            }
            Stmt::Assign { target, value, .. } => {
                let _ = self.type_of_expr(target);
                let _ = self.type_of_expr(value);
            }
            Stmt::ExprStmt(e) => {
                let _ = self.type_of_expr(e);
            }
            Stmt::Fail(e) | Stmt::Succeed(e) | Stmt::Result(e) | Stmt::Event(e) | Stmt::Await(e) => {
                let _ = self.type_of_expr(e);
            }
            Stmt::If {
                condition,
                then,
                else_,
            } => {
                let _ = self.type_of_expr(condition);
                self.symbols.push_scope();
                for s in then {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
                if let Some(else_body) = else_ {
                    self.symbols.push_scope();
                    for s in else_body {
                        self.check_stmt(s);
                    }
                    self.symbols.pop_scope();
                }
            }
            Stmt::Loop { body, .. } => {
                self.symbols.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
            }
            Stmt::Stop | Stmt::Skip => {}
            Stmt::Every { interval, body } => {
                let _ = self.type_of_expr(interval);
                self.symbols.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
            }
            Stmt::Match { subject, arms } => {
                let _ = self.type_of_expr(subject);
                for arm in arms {
                    self.check_match_arm(arm);
                }
            }
            Stmt::Task { body } => {
                self.symbols.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
            }
            Stmt::Cancel(_) => {}
        }
    }

    fn check_match_arm(&mut self, arm: &MatchArm) {
        if let Some(g) = &arm.guard {
            let _ = self.type_of_expr(g);
        }
        match &arm.pattern {
            MatchPattern::Literal(e) | MatchPattern::GuardedWildcard(e) => {
                let _ = self.type_of_expr(e);
            }
            _ => {}
        }
        self.symbols.push_scope();
        for s in &arm.body {
            self.check_stmt(s);
        }
        self.symbols.pop_scope();
    }

    fn type_of_expr(&mut self, e: &Expr) -> SkevType {
        match e {
            Expr::IntLiteral(_) => SkevType::Int,
            Expr::FloatLiteral(_) => SkevType::Float,
            Expr::BoolLiteral(_) => SkevType::Bool,
            Expr::StringLiteral(parts) => {
                for p in parts {
                    if let StringPart::Interpolation(inner) = p {
                        let _ = self.type_of_expr(inner);
                    }
                }
                SkevType::String
            }
            Expr::Identifier(name) => {
                if name == "self" {
                    return SkevType::Unknown;
                }
                if let Some(sym) = self.symbols.lookup(name) {
                    sym.ty.clone()
                } else {
                    self.error(format!("Undefined identifier '{}'", name));
                    SkevType::Unknown
                }
            }
            Expr::BinaryOp { left, op, right } => {
                let lt = self.type_of_expr(left);
                let rt = self.type_of_expr(right);
                match op {
                    BinOp::Eq
                    | BinOp::NotEq
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::LtEq
                    | BinOp::GtEq
                    | BinOp::And
                    | BinOp::Or
                    | BinOp::Is
                    | BinOp::Contains => SkevType::Bool,
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::WrapAdd
                    | BinOp::WrapSub
                    | BinOp::WrapMul
                    | BinOp::SatAdd
                    | BinOp::SatSub
                    | BinOp::SatMul
                    | BinOp::PanicAdd
                    | BinOp::PanicSub
                    | BinOp::PanicMul
                    | BinOp::Shl
                    | BinOp::Shr
                    | BinOp::Band
                    | BinOp::Bor
                    | BinOp::Bxor
                    | BinOp::OrElse => {
                        if lt == SkevType::Unknown {
                            rt
                        } else {
                            lt
                        }
                    }
                }
            }
            Expr::UnaryOp { op, expr } => {
                let t = self.type_of_expr(expr);
                match op {
                    UnaryOp::Neg => t,
                    UnaryOp::Not | UnaryOp::Exists => SkevType::Bool,
                    UnaryOp::BNot => t,
                }
            }
            Expr::Call { callee, args } => {
                let ct = self.type_of_expr(callee);
                for a in args {
                    let _ = self.type_of_expr(&a.value);
                }
                match ct {
                    SkevType::Fn { ret, .. } => *ret,
                    _ => SkevType::Unknown,
                }
            }
            Expr::FieldAccess { object, .. } => {
                let _ = self.type_of_expr(object);
                SkevType::Unknown
            }
            Expr::Index { object, index } => {
                let ot = self.type_of_expr(object);
                let _ = self.type_of_expr(index);
                match ot {
                    SkevType::List(t) => *t,
                    SkevType::Array(t, _) => *t,
                    SkevType::Map(_, v) => *v,
                    _ => SkevType::Unknown,
                }
            }
            Expr::Match { subject, arms } => {
                let _ = self.type_of_expr(subject);
                for arm in arms {
                    self.check_match_arm(arm);
                }
                SkevType::Unknown
            }
            Expr::IfExists {
                value,
                binding,
                then,
                else_,
            } => {
                let value_ty = self.type_of_expr(value);
                let unwrapped = match value_ty {
                    SkevType::Maybe(inner) => *inner,
                    other => other,
                };
                self.symbols.push_scope();
                self.symbols.define(Symbol {
                    name: binding.clone(),
                    ty: unwrapped,
                    kind: SymbolKind::Variable,
                    mutable: false,
                });
                for s in then {
                    self.check_stmt(s);
                }
                self.symbols.pop_scope();
                if let Some(else_body) = else_ {
                    self.symbols.push_scope();
                    for s in else_body {
                        self.check_stmt(s);
                    }
                    self.symbols.pop_scope();
                }
                SkevType::Unknown
            }
            Expr::Or { value, fallback } => {
                let vt = self.type_of_expr(value);
                let _ = self.type_of_expr(fallback);
                match vt {
                    SkevType::Maybe(inner) => *inner,
                    other => other,
                }
            }
            Expr::As { value, ty } => {
                let _ = self.type_of_expr(value);
                self.convert_type(ty)
            }
            Expr::ListLiteral(items) => {
                let inner = if let Some(first) = items.first() {
                    self.type_of_expr(first)
                } else {
                    SkevType::Unknown
                };
                for it in items.iter().skip(1) {
                    let _ = self.type_of_expr(it);
                }
                SkevType::List(Box::new(inner))
            }
            Expr::MapLiteral(pairs) => {
                for (k, v) in pairs {
                    let _ = self.type_of_expr(k);
                    let _ = self.type_of_expr(v);
                }
                SkevType::Map(Box::new(SkevType::Unknown), Box::new(SkevType::Unknown))
            }
            Expr::Contains { collection, item } => {
                let _ = self.type_of_expr(collection);
                let _ = self.type_of_expr(item);
                SkevType::Bool
            }
            Expr::Async(inner) | Expr::Arrow(inner) => self.type_of_expr(inner),
        }
    }
}

pub fn typecheck(program: &Program) -> Vec<TypeError> {
    let mut tc = TypeChecker::new();
    tc.register_top_level(program);
    tc.check_top_level(program);
    tc.errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn check(src: &str) -> Vec<TypeError> {
        let (tokens, _) = lex(src);
        let (program, _) = parse(tokens);
        typecheck(&program)
    }

    #[test]
    fn test_empty_program() {
        let errors = check("");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_entity_registered() {
        let errors = check("entity Player >>\n<< Player");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_fn_registered() {
        let errors = check("fn add(x: int, y: int) -> int >>\n<< add");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_undefined_variable() {
        let errors = check("fn f() >>\n    x = undefined_var\n<< f");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_int_literal_type() {
        let errors = check("fn f() >>\n    x :: int = 42\n<< f");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_bool_literal_type() {
        let errors = check("fn f() >>\n    x :: bool = true\n<< f");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_type_mismatch() {
        let errors = check("fn f() >>\n    x :: int = true\n<< f");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_data_registered() {
        let errors = check("data Point >>\n    x :: float\n    y :: float\n<< Point");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_kind_registered() {
        let errors = check("kind Direction >>\n    north\n    south\n<< Direction");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_two_entities_reference_each_other() {
        let src = "entity Player >>\n<< Player\nentity Enemy >>\n<< Enemy";
        let errors = check(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_maybe_type_valid() {
        let errors = check("fn f() -> maybe int >>\n<< f");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_result_type_valid() {
        let errors = check("fn f() -> result[int] >>\n<< f");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_async_fn_valid() {
        let errors = check("async fn load() >>\n<< load");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_entity_with_property() {
        let src = "entity Player >>\n    health :: int = 100\n<< Player";
        let errors = check(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_entity_with_when() {
        let src = "entity Player >>\n    when update(delta: float) >>\n    << update\n<< Player";
        let errors = check(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_typecheck_property_game_native_type() {
        let src = "entity Player >>\n    pos :: Vector3!\n<< Player";
        let errors = check(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_typecheck_param_game_native_type() {
        let src = "fn move_to(target: Vector3!) >>\n<< move_to";
        let errors = check(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_typecheck_return_game_native_type() {
        let src = "fn origin() -> Vector3! >>\n<< origin";
        let errors = check(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_typecheck_unknown_game_native_errors() {
        let src = "entity Player >>\n    pos :: Player!\n<< Player";
        let errors = check(src);
        assert!(
            errors.iter().any(|e| e.message.contains("Player!")),
            "Expected error mentioning 'Player!', got: {:?}",
            errors
        );
    }

    #[test]
    fn test_typecheck_known_game_native_no_error() {
        let src = "entity Mob >>\n    pos :: Vector3!\n<< Mob";
        let errors = check(src);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_typecheck_unknown_error_message_quality() {
        let src = "entity Spawner >>\n    target :: Enemy!\n<< Spawner";
        let errors = check(src);
        let msg = errors
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(msg.contains("Enemy"), "Expected 'Enemy' in message: {msg}");
        assert!(
            msg.contains("maybe Enemy"),
            "Expected 'maybe Enemy' suggestion in message: {msg}"
        );
    }
}
