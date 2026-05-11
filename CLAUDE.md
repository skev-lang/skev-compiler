# Skev Compiler — Claude Code Context

## What This Project Is
The official Skev programming language compiler.
Skev is a compiled, statically-typed game programming language.
Identity: "Fast like C++ and C#. Easy to read like Python."
Domains: skev.dev | skev.org
Copyright © 2026 AJ. All Rights Reserved.

## Tech Stack
- Language:  Rust 1.95.0
- Backend:   LLVM 18.1.8 via inkwell 0.9.0
- Target:    Native binaries — iOS, Android, macOS, Linux, Windows

## Project Structure
src/
├── main.rs          — entry point, CLI argument handling
├── lexer/mod.rs     — tokeniser: &str → Vec<Token>
├── parser/mod.rs    — AST builder: Vec<Token> → AST
├── typechecker/mod.rs — static type analysis
├── codegen/mod.rs   — LLVM IR generation
├── runtime/mod.rs   — ARC runtime implementation
└── stdlib/mod.rs    — standard library bindings
tests/               — 417 compliance tests (from Python transpiler)

## Compiler Pipeline
.skev source → Lexer → Parser → TypeChecker → CodeGen → Binary

## Build Commands
cargo build          — debug build
cargo test           — run all tests
cargo run -- file.skev — compile a Skev file

## Skev Language Rules — Critical
- Blocks open with >> and close with 
- NO indentation-based scoping — >> and << are explicit
- Entity model — not classes, not structs
- ARC memory — no garbage collector
- when handlers — not methods or callbacks
- :: for type annotation
- -> for return type
- kind for enums (not enum)
- data for pure data containers
- maybe TypeName for nullable types
- result[T] for error handling
- fail / succeed — not throw / return Ok

## Token Types (45 total)
Keywords: entity when fn data kind match if else loop task
          async await has import extern result fail succeed
          maybe shared realtime unsafe test mock every event
Operators: >> << :: -> => += -= *= /= == != <= >= < > + - * / = . , ! ?
Delimiters: ( ) [ ]
Literals: IntLiteral FloatLiteral StringStart StringContent
          InterpolationExpr StringEnd BoolLiteral
Special: Identifier DocComment EOF

## Error Handling Philosophy
- Collect ALL errors — never fail on first error
- Errors stored as Vec<LexError>, Vec<ParseError> etc.
- Always return (result, errors) tuple — never panic

## Compliance Target
The 417 tests from the Python transpiler are the
compiler's compliance suite. Every test must pass.
Test format: input .skev → expected output

## Current Phase
Phase A — Lexer (src/lexer/mod.rs)
Status: In progress