use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FunctionType, StructType};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};

use crate::parser::{
    AssignOp, BinOp, EntityItem, Expr, MatchArm, Param, Program, Stmt, StringPart, TopLevel,
    TypeExpr, UnaryOp,
};
#[allow(unused_imports)]
use crate::typechecker::SkevType;
use crate::types::game_native::{self, GameNativeLayout};

#[derive(Debug, Clone)]
pub struct CodegenError {
    pub message: String,
}

pub struct Codegen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    errors: Vec<CodegenError>,

    entity_structs: HashMap<String, StructType<'ctx>>,
    data_structs: HashMap<String, StructType<'ctx>>,
    functions: HashMap<String, FunctionValue<'ctx>>,
    locals: Vec<HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        let mut cg = Codegen {
            context,
            module,
            builder,
            errors: Vec::new(),
            entity_structs: HashMap::new(),
            data_structs: HashMap::new(),
            functions: HashMap::new(),
            locals: Vec::new(),
        };
        cg.declare_arc_runtime();
        cg
    }

    pub fn compile(&mut self, program: &Program) -> Vec<CodegenError> {
        self.register_types(program);
        self.fill_type_bodies(program);
        self.emit_type_markers();
        self.register_function_signatures(program);
        self.compile_function_bodies(program);
        self.emit_main();
        std::mem::take(&mut self.errors)
    }

    /// Force named struct types into the printed IR. Under LLVM opaque
    /// pointers an unused struct type can be elided; we keep the entity /
    /// data type name visible by attaching a `private global` of that
    /// struct type.
    fn emit_type_markers(&mut self) {
        let entity_names: Vec<String> = self.entity_structs.keys().cloned().collect();
        for name in entity_names {
            let st = self.entity_structs[&name];
            let global = self
                .module
                .add_global(st, None, &format!("{}_type_marker", name));
            global.set_initializer(&st.const_zero());
            global.set_linkage(inkwell::module::Linkage::Private);
        }
        let data_names: Vec<String> = self.data_structs.keys().cloned().collect();
        for name in data_names {
            let st = self.data_structs[&name];
            let global = self
                .module
                .add_global(st, None, &format!("{}_type_marker", name));
            global.set_initializer(&st.const_zero());
            global.set_linkage(inkwell::module::Linkage::Private);
        }
    }

    pub fn emit_llvm_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    // ---- ARC runtime ----

    fn declare_arc_runtime(&mut self) {
        let i64_t = self.context.i64_type();
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let void_t = self.context.void_type();

        let alloc_ty = ptr_t.fn_type(&[i64_t.into()], false);
        let alloc_fn = self.module.add_function("skev_alloc", alloc_ty, None);
        self.functions.insert("skev_alloc".to_string(), alloc_fn);

        let one_ptr: [BasicMetadataTypeEnum<'ctx>; 1] = [ptr_t.into()];
        let dealloc_ty = void_t.fn_type(&one_ptr, false);
        let dealloc_fn = self.module.add_function("skev_dealloc", dealloc_ty, None);
        self.functions
            .insert("skev_dealloc".to_string(), dealloc_fn);

        let retain_ty = void_t.fn_type(&one_ptr, false);
        let retain_fn = self.module.add_function("skev_retain", retain_ty, None);
        self.functions.insert("skev_retain".to_string(), retain_fn);

        let release_ty = void_t.fn_type(&one_ptr, false);
        let release_fn = self.module.add_function("skev_release", release_ty, None);
        self.functions
            .insert("skev_release".to_string(), release_fn);

        let no_args: [BasicMetadataTypeEnum<'ctx>; 0] = [];
        let init_ty = void_t.fn_type(&no_args, false);
        let init_fn = self.module.add_function("skev_init", init_ty, None);
        self.functions.insert("skev_init".to_string(), init_fn);

        let shutdown_ty = void_t.fn_type(&no_args, false);
        let shutdown_fn = self.module.add_function("skev_shutdown", shutdown_ty, None);
        self.functions
            .insert("skev_shutdown".to_string(), shutdown_fn);
    }

    // ---- Pass 1: register named types ----

    fn register_types(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Entity { name, .. } => {
                    let st = self.context.opaque_struct_type(name);
                    self.entity_structs.insert(name.clone(), st);
                }
                TopLevel::Data { name, .. } => {
                    let st = self.context.opaque_struct_type(name);
                    self.data_structs.insert(name.clone(), st);
                }
                _ => {}
            }
        }
    }

    // ---- Pass 2: fill struct bodies ----

    fn fill_type_bodies(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Entity { name, body } => {
                    let mut fields: Vec<BasicTypeEnum<'ctx>> =
                        vec![self.context.i64_type().as_basic_type_enum()];
                    for entity_item in body {
                        if let EntityItem::Property { ty, .. } = entity_item {
                            if let Some(ft) = self.skev_type_to_llvm(ty) {
                                fields.push(ft);
                            }
                        }
                    }
                    if let Some(st) = self.entity_structs.get(name) {
                        st.set_body(&fields, false);
                    }
                }
                TopLevel::Data { name, fields } => {
                    let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
                    for f in fields {
                        if let Some(ft) = self.skev_type_to_llvm(&f.ty) {
                            llvm_fields.push(ft);
                        }
                    }
                    if let Some(st) = self.data_structs.get(name) {
                        st.set_body(&llvm_fields, false);
                    }
                }
                _ => {}
            }
        }
    }

    // ---- Pass 3: register function signatures ----

    fn register_function_signatures(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Fn {
                    name, params, ret, ..
                } => {
                    let fn_type = self.build_fn_type(params, ret.as_ref(), None);
                    let fv = self.module.add_function(name, fn_type, None);
                    self.functions.insert(name.clone(), fv);
                }
                TopLevel::Entity {
                    name: entity_name,
                    body,
                } => {
                    for entity_item in body {
                        match entity_item {
                            EntityItem::When { event, params, .. } => {
                                let fn_name = format!("{}_when_{}", entity_name, event);
                                let fn_type =
                                    self.build_fn_type(params, None, Some(entity_name.as_str()));
                                let fv = self.module.add_function(&fn_name, fn_type, None);
                                self.functions.insert(fn_name, fv);
                            }
                            EntityItem::Method {
                                name, params, ret, ..
                            } => {
                                let fn_name = format!("{}_{}", entity_name, name);
                                let fn_type = self.build_fn_type(
                                    params,
                                    ret.as_ref(),
                                    Some(entity_name.as_str()),
                                );
                                let fv = self.module.add_function(&fn_name, fn_type, None);
                                self.functions.insert(fn_name, fv);
                            }
                            _ => {}
                        }
                    }
                }
                TopLevel::Extern { items, .. } => {
                    for ext_item in items {
                        let fn_type = self.build_fn_type(&ext_item.params, ext_item.ret.as_ref(), None);
                        let fv = self.module.add_function(&ext_item.name, fn_type, None);
                        self.functions.insert(ext_item.name.clone(), fv);
                    }
                }
                _ => {}
            }
        }
    }

    fn build_fn_type(
        &mut self,
        params: &[Param],
        ret: Option<&TypeExpr>,
        self_entity: Option<&str>,
    ) -> FunctionType<'ctx> {
        let mut param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::new();

        if self_entity.is_some() {
            let ptr_t = self.context.ptr_type(AddressSpace::default());
            param_tys.push(ptr_t.into());
        }

        for p in params {
            if let Some(ty) = self.skev_type_to_llvm(&p.ty) {
                param_tys.push(ty.into());
            } else {
                param_tys.push(self.context.i64_type().into());
            }
        }

        let void_t = self.context.void_type();
        match ret {
            Some(rt) => match self.skev_type_to_llvm(rt) {
                Some(ty) => match ty {
                    BasicTypeEnum::IntType(i) => i.fn_type(&param_tys, false),
                    BasicTypeEnum::FloatType(f) => f.fn_type(&param_tys, false),
                    BasicTypeEnum::StructType(s) => s.fn_type(&param_tys, false),
                    BasicTypeEnum::PointerType(p) => p.fn_type(&param_tys, false),
                    BasicTypeEnum::ArrayType(a) => a.fn_type(&param_tys, false),
                    BasicTypeEnum::VectorType(v) => v.fn_type(&param_tys, false),
                    _ => void_t.fn_type(&param_tys, false),
                },
                None => void_t.fn_type(&param_tys, false),
            },
            None => void_t.fn_type(&param_tys, false),
        }
    }

    // ---- Pass 4: compile bodies ----

    fn compile_function_bodies(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Fn {
                    name,
                    params,
                    ret,
                    body,
                    ..
                } => {
                    if let Some(fv) = self.functions.get(name).copied() {
                        self.compile_fn_body(fv, params, body, ret.as_ref(), false);
                    }
                }
                TopLevel::Entity {
                    name: entity_name,
                    body,
                } => {
                    for entity_item in body {
                        match entity_item {
                            EntityItem::When {
                                event,
                                params,
                                body: ev_body,
                                ..
                            } => {
                                let fn_name = format!("{}_when_{}", entity_name, event);
                                if let Some(fv) = self.functions.get(&fn_name).copied() {
                                    self.compile_fn_body(fv, params, ev_body, None, true);
                                }
                            }
                            EntityItem::Method {
                                name,
                                params,
                                ret,
                                body: mt_body,
                                ..
                            } => {
                                let fn_name = format!("{}_{}", entity_name, name);
                                if let Some(fv) = self.functions.get(&fn_name).copied() {
                                    self.compile_fn_body(fv, params, mt_body, ret.as_ref(), true);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn compile_fn_body(
        &mut self,
        fn_val: FunctionValue<'ctx>,
        params: &[Param],
        body: &[Stmt],
        ret: Option<&TypeExpr>,
        has_self: bool,
    ) {
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);

        self.locals.push(HashMap::new());

        let mut idx: u32 = 0;
        if has_self {
            if let Some(p) = fn_val.get_nth_param(idx) {
                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                let alloca = match self.builder.build_alloca(ptr_ty, "self.addr") {
                    Ok(a) => a,
                    Err(_) => {
                        self.locals.pop();
                        return;
                    }
                };
                let _ = self.builder.build_store(alloca, p);
                self.locals
                    .last_mut()
                    .unwrap()
                    .insert("self".to_string(), (alloca, ptr_ty.as_basic_type_enum()));
            }
            idx += 1;
        }

        for p in params {
            let llvm_ty = self
                .skev_type_to_llvm(&p.ty)
                .unwrap_or_else(|| self.context.i64_type().as_basic_type_enum());
            if let Some(pv) = fn_val.get_nth_param(idx) {
                let alloca = match self.builder.build_alloca(llvm_ty, &format!("{}.addr", p.name)) {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                let _ = self.builder.build_store(alloca, pv);
                self.locals
                    .last_mut()
                    .unwrap()
                    .insert(p.name.clone(), (alloca, llvm_ty));
            }
            idx += 1;
        }

        for stmt in body {
            self.compile_stmt(stmt);
        }

        let needs_terminator = self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_terminator())
            .is_none();

        if needs_terminator {
            self.emit_default_return(ret);
        }

        self.locals.pop();
    }

    fn emit_default_return(&mut self, ret: Option<&TypeExpr>) {
        match ret.and_then(|rt| self.skev_type_to_llvm(rt)) {
            Some(BasicTypeEnum::IntType(i)) => {
                let _ = self.builder.build_return(Some(&i.const_zero()));
            }
            Some(BasicTypeEnum::FloatType(f)) => {
                let _ = self.builder.build_return(Some(&f.const_zero()));
            }
            Some(BasicTypeEnum::PointerType(p)) => {
                let _ = self.builder.build_return(Some(&p.const_null()));
            }
            Some(BasicTypeEnum::StructType(s)) => {
                let _ = self.builder.build_return(Some(&s.const_zero()));
            }
            Some(BasicTypeEnum::ArrayType(a)) => {
                let _ = self.builder.build_return(Some(&a.const_zero()));
            }
            Some(BasicTypeEnum::VectorType(v)) => {
                let _ = self.builder.build_return(Some(&v.const_zero()));
            }
            _ => {
                let _ = self.builder.build_return(None);
            }
        }
    }

    // ---- Main entry point ----

    fn emit_main(&mut self) {
        if self.module.get_function("main").is_some() {
            return;
        }
        let i32_t = self.context.i32_type();
        let no_args: [BasicMetadataTypeEnum<'ctx>; 0] = [];
        let main_ty = i32_t.fn_type(&no_args, false);
        let main_fn = self.module.add_function("main", main_ty, None);
        let entry = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(entry);

        if let Some(init_fn) = self.functions.get("skev_init").copied() {
            let _ = self.builder.build_call(init_fn, &[], "");
        }
        if let Some(shutdown_fn) = self.functions.get("skev_shutdown").copied() {
            let _ = self.builder.build_call(shutdown_fn, &[], "");
        }
        let _ = self.builder.build_return(Some(&i32_t.const_zero()));
    }

    // ---- Type conversion ----

    fn skev_type_to_llvm(&mut self, te: &TypeExpr) -> Option<BasicTypeEnum<'ctx>> {
        let ctx = self.context;
        match te {
            TypeExpr::Named(s) => match s.as_str() {
                "int" | "int64" => Some(ctx.i64_type().as_basic_type_enum()),
                "int32" => Some(ctx.i32_type().as_basic_type_enum()),
                "int16" => Some(ctx.i16_type().as_basic_type_enum()),
                "int8" | "uint8" => Some(ctx.i8_type().as_basic_type_enum()),
                "uint16" => Some(ctx.i16_type().as_basic_type_enum()),
                "uint32" => Some(ctx.i32_type().as_basic_type_enum()),
                "uint64" => Some(ctx.i64_type().as_basic_type_enum()),
                "float" | "float32" => Some(ctx.f32_type().as_basic_type_enum()),
                "float64" => Some(ctx.f64_type().as_basic_type_enum()),
                "bool" => Some(ctx.bool_type().as_basic_type_enum()),
                "string" => Some(self.string_struct().as_basic_type_enum()),
                "nothing" => None,
                "<inferred>" | "<error>" => Some(ctx.i64_type().as_basic_type_enum()),
                name => {
                    if self.entity_structs.contains_key(name) {
                        Some(ctx.ptr_type(AddressSpace::default()).as_basic_type_enum())
                    } else if let Some(s) = self.data_structs.get(name) {
                        Some(s.as_basic_type_enum())
                    } else {
                        Some(ctx.ptr_type(AddressSpace::default()).as_basic_type_enum())
                    }
                }
            },
            TypeExpr::Maybe(t) | TypeExpr::Result(t) => {
                let inner = self
                    .skev_type_to_llvm(t)
                    .unwrap_or_else(|| ctx.i64_type().as_basic_type_enum());
                let s = ctx.struct_type(&[ctx.bool_type().into(), inner], false);
                Some(s.as_basic_type_enum())
            }
            TypeExpr::List(_) => Some(self.list_struct().as_basic_type_enum()),
            TypeExpr::Channel(_) => Some(ctx.ptr_type(AddressSpace::default()).as_basic_type_enum()),
            TypeExpr::GameNative(s) => {
                let f32t = ctx.f32_type();
                let st = match game_native::layout_for(s) {
                    Some(GameNativeLayout::Vec2F32) => {
                        ctx.struct_type(&[f32t.into(), f32t.into()], false)
                    }
                    Some(GameNativeLayout::Vec3F32) => {
                        ctx.struct_type(&[f32t.into(), f32t.into(), f32t.into()], false)
                    }
                    Some(GameNativeLayout::Vec4F32)
                    | Some(GameNativeLayout::QuatF32)
                    | Some(GameNativeLayout::ColorF32)
                    | Some(GameNativeLayout::RectF32) => ctx.struct_type(
                        &[f32t.into(), f32t.into(), f32t.into(), f32t.into()],
                        false,
                    ),
                    Some(GameNativeLayout::RayF32) => ctx.struct_type(
                        &[
                            f32t.into(),
                            f32t.into(),
                            f32t.into(),
                            f32t.into(),
                            f32t.into(),
                            f32t.into(),
                        ],
                        false,
                    ),
                    Some(GameNativeLayout::Transform) => {
                        let row = f32t.array_type(4);
                        ctx.struct_type(
                            &[row.into(), row.into(), row.into(), row.into()],
                            false,
                        )
                    }
                    Some(GameNativeLayout::Matrix4F32) => {
                        let arr = f32t.array_type(16);
                        ctx.struct_type(&[arr.into()], false)
                    }
                    None => panic!(
                        "codegen reached unknown game-native type '{}' \
                         — typechecker should have rejected this",
                        s
                    ),
                };
                Some(st.as_basic_type_enum())
            }
            TypeExpr::Array { ty, size } => {
                let inner = self
                    .skev_type_to_llvm(ty)
                    .unwrap_or_else(|| ctx.i64_type().as_basic_type_enum());
                let arr = match inner {
                    BasicTypeEnum::IntType(i) => i.array_type(*size as u32).as_basic_type_enum(),
                    BasicTypeEnum::FloatType(f) => f.array_type(*size as u32).as_basic_type_enum(),
                    BasicTypeEnum::StructType(s) => s.array_type(*size as u32).as_basic_type_enum(),
                    BasicTypeEnum::PointerType(p) => p.array_type(*size as u32).as_basic_type_enum(),
                    BasicTypeEnum::ArrayType(a) => a.array_type(*size as u32).as_basic_type_enum(),
                    BasicTypeEnum::VectorType(v) => v.array_type(*size as u32).as_basic_type_enum(),
                    BasicTypeEnum::ScalableVectorType(v) => {
                        v.array_type(*size as u32).as_basic_type_enum()
                    }
                };
                Some(arr)
            }
            TypeExpr::Generic { base, args } => match base.as_str() {
                "list" => Some(self.list_struct().as_basic_type_enum()),
                "result" | "maybe" if args.len() == 1 => {
                    let inner = self
                        .skev_type_to_llvm(&args[0])
                        .unwrap_or_else(|| ctx.i64_type().as_basic_type_enum());
                    Some(
                        ctx.struct_type(&[ctx.bool_type().into(), inner], false)
                            .as_basic_type_enum(),
                    )
                }
                _ => Some(ctx.ptr_type(AddressSpace::default()).as_basic_type_enum()),
            },
        }
    }

    fn string_struct(&self) -> StructType<'ctx> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i64_t = self.context.i64_type();
        self.context.struct_type(&[ptr_t.into(), i64_t.into()], false)
    }

    fn list_struct(&self) -> StructType<'ctx> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i64_t = self.context.i64_type();
        self.context
            .struct_type(&[ptr_t.into(), i64_t.into(), i64_t.into()], false)
    }

    // ---- Statements ----

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { name, ty, value } => {
                let llvm_ty = ty
                    .as_ref()
                    .and_then(|t| self.skev_type_to_llvm(t))
                    .or_else(|| {
                        value.as_ref().and_then(|_| {
                            Some(self.context.i64_type().as_basic_type_enum())
                        })
                    })
                    .unwrap_or_else(|| self.context.i64_type().as_basic_type_enum());

                let alloca = match self.builder.build_alloca(llvm_ty, name) {
                    Ok(a) => a,
                    Err(_) => return,
                };

                if let Some(v) = value {
                    if let Some(val) = self.compile_expr(v) {
                        let coerced = self.coerce_value(val, llvm_ty);
                        let _ = self.builder.build_store(alloca, coerced);
                    }
                }

                self.locals
                    .last_mut()
                    .unwrap()
                    .insert(name.clone(), (alloca, llvm_ty));
            }
            Stmt::Assign { target, op, value } => {
                if let Expr::Identifier(name) = target {
                    let local = self
                        .locals
                        .iter()
                        .rev()
                        .find_map(|s| s.get(name))
                        .copied();
                    if let Some((alloca, ty)) = local {
                        let new_val = self.compile_expr(value);
                        if let Some(nv) = new_val {
                            let coerced = self.coerce_value(nv, ty);
                            match op {
                                AssignOp::Eq => {
                                    let _ = self.builder.build_store(alloca, coerced);
                                }
                                _ => {
                                    if let Ok(cur) = self.builder.build_load(ty, alloca, "tmp") {
                                        let bin_op = match op {
                                            AssignOp::PlusEq => BinOp::Add,
                                            AssignOp::MinusEq => BinOp::Sub,
                                            AssignOp::StarEq => BinOp::Mul,
                                            AssignOp::SlashEq => BinOp::Div,
                                            AssignOp::WrapAddEq => BinOp::WrapAdd,
                                            AssignOp::WrapSubEq => BinOp::WrapSub,
                                            AssignOp::WrapMulEq => BinOp::WrapMul,
                                            AssignOp::SatAddEq => BinOp::SatAdd,
                                            AssignOp::SatSubEq => BinOp::SatSub,
                                            AssignOp::SatMulEq => BinOp::SatMul,
                                            AssignOp::PanicAddEq => BinOp::PanicAdd,
                                            AssignOp::PanicSubEq => BinOp::PanicSub,
                                            AssignOp::PanicMulEq => BinOp::PanicMul,
                                            AssignOp::Eq => unreachable!(),
                                        };
                                        if let Some(combined) =
                                            self.apply_binary_op(bin_op, cur, coerced)
                                        {
                                            let _ = self.builder.build_store(alloca, combined);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        let _ = self.compile_expr(value);
                    }
                } else {
                    let _ = self.compile_expr(target);
                    let _ = self.compile_expr(value);
                }
            }
            Stmt::ExprStmt(e) => {
                let _ = self.compile_expr(e);
            }
            Stmt::Fail(e) | Stmt::Succeed(e) | Stmt::Result(e) => {
                if let Some(v) = self.compile_expr(e) {
                    let _ = self.builder.build_return(Some(&v));
                } else {
                    let _ = self.builder.build_return(None);
                }
            }
            Stmt::Event(e) | Stmt::Await(e) => {
                let _ = self.compile_expr(e);
            }
            Stmt::If {
                condition,
                then,
                else_,
            } => {
                self.compile_if(condition, then, else_.as_deref());
            }
            Stmt::Loop { body, .. } => {
                self.compile_loop(body);
            }
            Stmt::Stop | Stmt::Skip => {}
            Stmt::Every { interval, body } => {
                let _ = self.compile_expr(interval);
                self.locals.push(HashMap::new());
                for s in body {
                    self.compile_stmt(s);
                }
                self.locals.pop();
            }
            Stmt::Match { subject, arms } => {
                let _ = self.compile_expr(subject);
                for arm in arms {
                    self.compile_match_arm(arm);
                }
            }
            Stmt::Task { body } => {
                self.locals.push(HashMap::new());
                for s in body {
                    self.compile_stmt(s);
                }
                self.locals.pop();
            }
            Stmt::Cancel(_) => {}
        }
    }

    fn compile_match_arm(&mut self, arm: &MatchArm) {
        self.locals.push(HashMap::new());
        for s in &arm.body {
            self.compile_stmt(s);
        }
        self.locals.pop();
    }

    fn compile_if(&mut self, condition: &Expr, then: &[Stmt], else_: Option<&[Stmt]>) {
        let cond_val = self.compile_expr(condition);
        let cond_bool = self.value_to_bool(cond_val);

        let parent_fn = match self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
        {
            Some(f) => f,
            None => return,
        };

        let then_bb = self.context.append_basic_block(parent_fn, "if.then");
        let merge_bb = self.context.append_basic_block(parent_fn, "if.merge");
        let else_bb = if else_.is_some() {
            self.context.append_basic_block(parent_fn, "if.else")
        } else {
            merge_bb
        };

        let _ = self
            .builder
            .build_conditional_branch(cond_bool, then_bb, else_bb);

        self.builder.position_at_end(then_bb);
        self.locals.push(HashMap::new());
        for s in then {
            self.compile_stmt(s);
        }
        self.locals.pop();
        if then_bb.get_terminator().is_none() {
            let _ = self.builder.build_unconditional_branch(merge_bb);
        }

        if let Some(ebody) = else_ {
            self.builder.position_at_end(else_bb);
            self.locals.push(HashMap::new());
            for s in ebody {
                self.compile_stmt(s);
            }
            self.locals.pop();
            if else_bb.get_terminator().is_none() {
                let _ = self.builder.build_unconditional_branch(merge_bb);
            }
        }

        self.builder.position_at_end(merge_bb);
    }

    fn compile_loop(&mut self, body: &[Stmt]) {
        let parent_fn = match self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
        {
            Some(f) => f,
            None => return,
        };
        let loop_bb = self.context.append_basic_block(parent_fn, "loop");
        let after_bb = self.context.append_basic_block(parent_fn, "loop.after");

        let _ = self.builder.build_unconditional_branch(loop_bb);

        self.builder.position_at_end(loop_bb);
        self.locals.push(HashMap::new());
        for s in body {
            self.compile_stmt(s);
        }
        self.locals.pop();
        if loop_bb.get_terminator().is_none() {
            let _ = self.builder.build_unconditional_branch(loop_bb);
        }
        self.builder.position_at_end(after_bb);
    }

    fn value_to_bool(
        &self,
        v: Option<BasicValueEnum<'ctx>>,
    ) -> inkwell::values::IntValue<'ctx> {
        let bool_t = self.context.bool_type();
        match v {
            Some(BasicValueEnum::IntValue(iv)) => {
                if iv.get_type().get_bit_width() == 1 {
                    iv
                } else {
                    let zero = iv.get_type().const_zero();
                    self.builder
                        .build_int_compare(IntPredicate::NE, iv, zero, "tobool")
                        .unwrap_or_else(|_| bool_t.const_zero())
                }
            }
            _ => bool_t.const_zero(),
        }
    }

    // ---- Expressions ----

    fn compile_expr(&mut self, expr: &Expr) -> Option<BasicValueEnum<'ctx>> {
        match expr {
            Expr::IntLiteral(n) => {
                Some(self.context.i64_type().const_int(*n as u64, true).into())
            }
            Expr::FloatLiteral(f) => Some(self.context.f32_type().const_float(*f).into()),
            Expr::BoolLiteral(b) => Some(
                self.context
                    .bool_type()
                    .const_int(if *b { 1 } else { 0 }, false)
                    .into(),
            ),
            Expr::StringLiteral(parts) => {
                for p in parts {
                    if let StringPart::Interpolation(inner) = p {
                        let _ = self.compile_expr(inner);
                    }
                }
                Some(self.string_struct().const_zero().into())
            }
            Expr::Identifier(name) => {
                let local = self
                    .locals
                    .iter()
                    .rev()
                    .find_map(|s| s.get(name))
                    .copied();
                if let Some((alloca, ty)) = local {
                    self.builder.build_load(ty, alloca, name).ok()
                } else if let Some(fv) = self.functions.get(name).copied() {
                    Some(fv.as_global_value().as_pointer_value().into())
                } else {
                    Some(self.context.i64_type().const_zero().into())
                }
            }
            Expr::BinaryOp { left, op, right } => {
                let lv = self.compile_expr(left)?;
                let rv = self.compile_expr(right)?;
                self.apply_binary_op(*op, lv, rv)
            }
            Expr::UnaryOp { op, expr } => {
                let v = self.compile_expr(expr)?;
                match op {
                    UnaryOp::Neg => match v {
                        BasicValueEnum::IntValue(iv) => {
                            Some(self.builder.build_int_neg(iv, "neg").ok()?.into())
                        }
                        BasicValueEnum::FloatValue(fv) => {
                            Some(self.builder.build_float_neg(fv, "fneg").ok()?.into())
                        }
                        other => Some(other),
                    },
                    UnaryOp::Not => match v {
                        BasicValueEnum::IntValue(iv) => {
                            Some(self.builder.build_not(iv, "not").ok()?.into())
                        }
                        other => Some(other),
                    },
                }
            }
            Expr::Call { callee, args } => {
                let fn_val = match callee.as_ref() {
                    Expr::Identifier(name) => self.functions.get(name).copied(),
                    Expr::FieldAccess { field, .. } => self.functions.get(field).copied(),
                    _ => None,
                };

                let mut arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::new();
                for a in args {
                    if let Some(av) = self.compile_expr(&a.value) {
                        arg_vals.push(av.into());
                    }
                }

                if let Some(fv) = fn_val {
                    let call = self.builder.build_call(fv, &arg_vals, "call").ok()?;
                    call.try_as_basic_value()
                        .basic()
                        .or_else(|| Some(self.context.i64_type().const_zero().into()))
                } else {
                    Some(self.context.i64_type().const_zero().into())
                }
            }
            Expr::FieldAccess { object, .. } => {
                let _ = self.compile_expr(object);
                Some(self.context.i64_type().const_zero().into())
            }
            Expr::Index { object, index } => {
                let _ = self.compile_expr(object);
                let _ = self.compile_expr(index);
                Some(self.context.i64_type().const_zero().into())
            }
            Expr::Match { subject, arms } => {
                let v = self.compile_expr(subject);
                for arm in arms {
                    self.compile_match_arm(arm);
                }
                v.or_else(|| Some(self.context.i64_type().const_zero().into()))
            }
            Expr::IfExists {
                value,
                then,
                else_,
                ..
            } => {
                let v = self.compile_expr(value);
                self.locals.push(HashMap::new());
                for s in then {
                    self.compile_stmt(s);
                }
                self.locals.pop();
                if let Some(else_body) = else_ {
                    self.locals.push(HashMap::new());
                    for s in else_body {
                        self.compile_stmt(s);
                    }
                    self.locals.pop();
                }
                v.or_else(|| Some(self.context.i64_type().const_zero().into()))
            }
            Expr::Or { value, fallback } => {
                let v = self.compile_expr(value);
                let _ = self.compile_expr(fallback);
                v.or_else(|| Some(self.context.i64_type().const_zero().into()))
            }
            Expr::As { value, ty } => {
                let v = self.compile_expr(value)?;
                let target_ty = self
                    .skev_type_to_llvm(ty)
                    .unwrap_or_else(|| self.context.i64_type().as_basic_type_enum());
                Some(self.coerce_value(v, target_ty))
            }
            Expr::ListLiteral(items) => {
                for it in items {
                    let _ = self.compile_expr(it);
                }
                Some(self.list_struct().const_zero().into())
            }
            Expr::MapLiteral(pairs) => {
                for (k, v) in pairs {
                    let _ = self.compile_expr(k);
                    let _ = self.compile_expr(v);
                }
                Some(
                    self.context
                        .ptr_type(AddressSpace::default())
                        .const_null()
                        .into(),
                )
            }
            Expr::Contains { collection, item } => {
                let _ = self.compile_expr(collection);
                let _ = self.compile_expr(item);
                Some(self.context.bool_type().const_zero().into())
            }
            Expr::Async(inner) | Expr::Arrow(inner) => self.compile_expr(inner),
        }
    }

    fn apply_binary_op(
        &self,
        op: BinOp,
        lv: BasicValueEnum<'ctx>,
        rv: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        match (lv, rv) {
            (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) => {
                let (l, r) = self.unify_int_widths(l, r);
                match op {
                    BinOp::Add | BinOp::WrapAdd | BinOp::SatAdd | BinOp::PanicAdd => {
                        Some(self.builder.build_int_add(l, r, "add").ok()?.into())
                    }
                    BinOp::Sub | BinOp::WrapSub | BinOp::SatSub | BinOp::PanicSub => {
                        Some(self.builder.build_int_sub(l, r, "sub").ok()?.into())
                    }
                    BinOp::Mul | BinOp::WrapMul | BinOp::SatMul | BinOp::PanicMul => {
                        Some(self.builder.build_int_mul(l, r, "mul").ok()?.into())
                    }
                    BinOp::Div => Some(
                        self.builder
                            .build_int_signed_div(l, r, "div")
                            .ok()?
                            .into(),
                    ),
                    BinOp::Eq => Some(
                        self.builder
                            .build_int_compare(IntPredicate::EQ, l, r, "eq")
                            .ok()?
                            .into(),
                    ),
                    BinOp::NotEq => Some(
                        self.builder
                            .build_int_compare(IntPredicate::NE, l, r, "ne")
                            .ok()?
                            .into(),
                    ),
                    BinOp::Lt => Some(
                        self.builder
                            .build_int_compare(IntPredicate::SLT, l, r, "lt")
                            .ok()?
                            .into(),
                    ),
                    BinOp::Gt => Some(
                        self.builder
                            .build_int_compare(IntPredicate::SGT, l, r, "gt")
                            .ok()?
                            .into(),
                    ),
                    BinOp::LtEq => Some(
                        self.builder
                            .build_int_compare(IntPredicate::SLE, l, r, "le")
                            .ok()?
                            .into(),
                    ),
                    BinOp::GtEq => Some(
                        self.builder
                            .build_int_compare(IntPredicate::SGE, l, r, "ge")
                            .ok()?
                            .into(),
                    ),
                    BinOp::And => Some(self.builder.build_and(l, r, "and").ok()?.into()),
                    BinOp::Or => Some(self.builder.build_or(l, r, "or").ok()?.into()),
                }
            }
            (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) => match op {
                BinOp::Add | BinOp::WrapAdd | BinOp::SatAdd | BinOp::PanicAdd => {
                    Some(self.builder.build_float_add(l, r, "fadd").ok()?.into())
                }
                BinOp::Sub | BinOp::WrapSub | BinOp::SatSub | BinOp::PanicSub => {
                    Some(self.builder.build_float_sub(l, r, "fsub").ok()?.into())
                }
                BinOp::Mul | BinOp::WrapMul | BinOp::SatMul | BinOp::PanicMul => {
                    Some(self.builder.build_float_mul(l, r, "fmul").ok()?.into())
                }
                BinOp::Div => Some(self.builder.build_float_div(l, r, "fdiv").ok()?.into()),
                BinOp::Eq => Some(
                    self.builder
                        .build_float_compare(FloatPredicate::OEQ, l, r, "feq")
                        .ok()?
                        .into(),
                ),
                BinOp::NotEq => Some(
                    self.builder
                        .build_float_compare(FloatPredicate::ONE, l, r, "fne")
                        .ok()?
                        .into(),
                ),
                BinOp::Lt => Some(
                    self.builder
                        .build_float_compare(FloatPredicate::OLT, l, r, "flt")
                        .ok()?
                        .into(),
                ),
                BinOp::Gt => Some(
                    self.builder
                        .build_float_compare(FloatPredicate::OGT, l, r, "fgt")
                        .ok()?
                        .into(),
                ),
                BinOp::LtEq => Some(
                    self.builder
                        .build_float_compare(FloatPredicate::OLE, l, r, "fle")
                        .ok()?
                        .into(),
                ),
                BinOp::GtEq => Some(
                    self.builder
                        .build_float_compare(FloatPredicate::OGE, l, r, "fge")
                        .ok()?
                        .into(),
                ),
                _ => Some(lv),
            },
            _ => Some(lv),
        }
    }

    fn unify_int_widths(
        &self,
        l: inkwell::values::IntValue<'ctx>,
        r: inkwell::values::IntValue<'ctx>,
    ) -> (inkwell::values::IntValue<'ctx>, inkwell::values::IntValue<'ctx>) {
        let lw = l.get_type().get_bit_width();
        let rw = r.get_type().get_bit_width();
        if lw == rw {
            return (l, r);
        }
        let target_ty = if lw > rw { l.get_type() } else { r.get_type() };
        let l = if lw < target_ty.get_bit_width() {
            self.builder
                .build_int_s_extend(l, target_ty, "sext")
                .unwrap_or(l)
        } else {
            l
        };
        let r = if rw < target_ty.get_bit_width() {
            self.builder
                .build_int_s_extend(r, target_ty, "sext")
                .unwrap_or(r)
        } else {
            r
        };
        (l, r)
    }

    fn coerce_value(
        &self,
        v: BasicValueEnum<'ctx>,
        target: BasicTypeEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match (v, target) {
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::IntType(it)) => {
                let vw = iv.get_type().get_bit_width();
                let tw = it.get_bit_width();
                if vw == tw {
                    iv.into()
                } else if vw < tw {
                    self.builder
                        .build_int_s_extend(iv, it, "sext")
                        .map(Into::into)
                        .unwrap_or(v)
                } else {
                    self.builder
                        .build_int_truncate(iv, it, "trunc")
                        .map(Into::into)
                        .unwrap_or(v)
                }
            }
            (BasicValueEnum::FloatValue(fv), BasicTypeEnum::FloatType(_ft)) => fv.into(),
            _ => v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn compile_src(src: &str) -> (String, Vec<CodegenError>) {
        let context = Context::create();
        let mut codegen = Codegen::new(&context, "test");
        let (tokens, _) = lex(src);
        let (program, _) = parse(tokens);
        let tc_errors = crate::typechecker::typecheck(&program);
        let mut errors: Vec<CodegenError> = tc_errors
            .into_iter()
            .map(|e| CodegenError { message: e.message })
            .collect();
        if errors.is_empty() {
            errors.extend(codegen.compile(&program));
        } else {
            // Don't run codegen when typecheck failed — the catch-all is a
            // compiler-bug panic guard.
            let _ = codegen.compile(&Program { items: vec![] });
        }
        let ir = codegen.emit_llvm_ir();
        (ir, errors)
    }

    #[test]
    fn test_empty_program_compiles() {
        let (ir, errors) = compile_src("");
        assert!(errors.is_empty());
        assert!(ir.contains("skev_alloc") ||
                ir.contains("ModuleID"));
    }

    #[test]
    fn test_entity_emits_struct() {
        let src = "entity Player >>\n    health :: int = 100\n<< Player";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("Player") ||
                ir.contains("%Player"));
    }

    #[test]
    fn test_fn_emits_function() {
        let src = "fn add(x: int, y: int) -> int >>\n<< add";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("@add"));
    }

    #[test]
    fn test_when_handler_emits_function() {
        let src = "entity Player >>\n    when update(delta: float) >>\n    << update\n<< Player";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("update"));
    }

    #[test]
    fn test_arc_runtime_declared() {
        let (ir, errors) = compile_src("");
        assert!(errors.is_empty());
        assert!(ir.contains("skev_alloc"));
        assert!(ir.contains("skev_retain"));
        assert!(ir.contains("skev_release"));
    }

    #[test]
    fn test_extern_block_emits_declaration() {
        let src = "extern \"C\" Raylib >>\n    init_window(width: int, height: int)\n<< Raylib";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("init_window"));
    }

    #[test]
    fn test_int_literal_compiles() {
        let src = "fn f() >>\n    x :: int = 42\n<< f";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("@f"));
    }

    #[test]
    fn test_binary_op_compiles() {
        let src = "fn add(x: int, y: int) -> int >>\n    result x + y\n<< add";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("add") || ir.contains("@add"));
    }

    #[test]
    fn test_if_compiles() {
        let src = "fn f(x: int) >>\n    if x == 1 >>\n    << x == 1\n<< f";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_data_type_emits_struct() {
        let src = "data Point >>\n    x :: float\n    y :: float\n<< Point";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(ir.contains("Point") ||
                ir.contains("%Point"));
    }

    #[test]
    fn test_emit_llvm_ir_returns_string() {
        let (ir, _) = compile_src("");
        assert!(!ir.is_empty());
    }

    #[test]
    fn test_codegen_vector3_layout() {
        let src = "entity Player >>\n    pos :: Vector3!\n<< Player";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(
            ir.contains("float, float, float"),
            "IR should contain Vector3! struct layout (3 × f32), got:\n{}",
            ir
        );
    }

    #[test]
    fn test_codegen_color_layout() {
        let src = "entity Player >>\n    tint :: Color!\n<< Player";
        let (ir, errors) = compile_src(src);
        assert!(errors.is_empty());
        assert!(
            ir.contains("float, float, float, float"),
            "IR should contain Color! struct layout, got:\n{}",
            ir
        );
    }

    #[test]
    fn test_codegen_unknown_game_native_now_rejected_at_typecheck() {
        let src = "entity Player >>\n    mesh :: RenderMesh!\n<< Player";
        let (_ir, errors) = compile_src(src);
        assert!(!errors.is_empty());
        assert!(
            errors.iter().any(|e| e.message.contains("RenderMesh!")),
            "Expected typecheck error mentioning 'RenderMesh!', got: {:?}",
            errors
        );
    }
}
