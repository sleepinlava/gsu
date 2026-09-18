mod accumulation;
mod bindings;
mod calls;
pub(crate) mod facts;
mod supported;
use crate::{
    diagnostic::{Diagnostic, Evidence, Location, OperationError},
    rules,
    source::Source,
};
use bindings::Bindings;
use facts::{Conversion, Device, Fact, Tensor};
use rustpython_ast::{self as ast, Visitor};

fn literal_dimension(e: &ast::Expr) -> Option<i64> {
    match e {
        ast::Expr::Constant(c) => match &c.value {
            ast::Constant::Int(n) => n.to_string().parse().ok(),
            _ => None,
        },
        ast::Expr::UnaryOp(u) => {
            let n = literal_dimension(&u.operand)?;
            match u.op {
                ast::UnaryOp::USub => n.checked_neg(),
                ast::UnaryOp::UAdd => Some(n),
                _ => None,
            }
        }
        _ => None,
    }
}
use rustpython_parser::text_size::TextRange;
use std::collections::{BTreeMap, BTreeSet};
type Env = BTreeMap<String, Fact>;
pub fn analyze(
    source: &Source,
    body: &[ast::Stmt],
    enabled: &BTreeSet<String>,
) -> Result<Vec<Diagnostic>, OperationError> {
    let mut analyzer = Analyzer {
        source,
        enabled,
        env: ["bool", "int", "float"]
            .into_iter()
            .map(|n| (n.to_string(), Fact::Name(format!("builtins.{n}"))))
            .collect(),
        external: BTreeSet::new(),
        class_outer: None,
        loop_depth: 0,
        epoch: 0,
        next_id: 0,
        diagnostics: vec![],
        overflow: false,
    };
    analyzer.block(body);
    if analyzer.overflow {
        Err(OperationError::new(
            "GSU-E008",
            "Single-file diagnostic limit exceeded",
            Some(source.path.clone()),
        ))
    } else {
        Ok(analyzer.diagnostics)
    }
}
struct Analyzer<'a> {
    source: &'a Source,
    enabled: &'a BTreeSet<String>,
    env: Env,
    external: BTreeSet<String>,
    class_outer: Option<Env>,
    loop_depth: usize,
    epoch: usize,
    next_id: usize,
    diagnostics: Vec<Diagnostic>,
    overflow: bool,
}
impl Analyzer<'_> {
    fn block(&mut self, body: &[ast::Stmt]) {
        for stmt in body {
            if self.overflow {
                break;
            }
            self.visit_stmt(stmt.clone());
        }
    }
    fn barrier(&mut self) {
        self.epoch += 1;
    }
    fn emit(
        &mut self,
        code: &'static str,
        range: TextRange,
        confidence: &'static str,
        detail: String,
    ) {
        if !self.enabled.contains(code) {
            return;
        }
        if self.diagnostics.len() >= 100_000 {
            self.overflow = true;
            return;
        }
        let r = rules::metadata(code).expect("registered rule");
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        let (excerpt, underline) = self.source.excerpt(start, end);
        let mut evidence = vec![Evidence {
            kind: "static_pattern".into(),
            detail,
        }];
        if self.loop_depth > 0 {
            evidence.push(Evidence {
                kind: "loop_context".into(),
                detail: "Repeated syntactic context; iteration count is unknown".into(),
            });
        }
        self.diagnostics.push(Diagnostic {
            rule: code,
            severity: if code == "S002" && self.loop_depth > 0 {
                "high"
            } else {
                r.severity
            },
            confidence,
            location: Location {
                path: self.source.path.clone(),
                start: self.source.position(start),
                end: self.source.position(end),
            },
            message: r.message,
            explanation: r.explanation,
            suggestion: r.suggestion,
            evidence,
            excerpt,
            underline,
        });
    }
    fn hardcoded(&mut self, device: &Option<(Device, Option<TextRange>)>) {
        if let Some((Device::Cuda(Some(index)), Some(range))) = device {
            self.emit(
                "D001",
                *range,
                "high",
                format!("Explicit CUDA index {index}"),
            );
        }
    }
    fn fresh(&mut self, device: Option<Device>, dtype: Option<String>, declared: bool) -> Fact {
        self.next_id += 1;
        Fact::Tensor(Tensor {
            id: self.next_id,
            device,
            dtype,
            declared,
            escaped: false,
            previous: None,
        })
    }
    fn put(&mut self, name: String, value: Fact) {
        self.env.insert(
            name.clone(),
            if self.external.contains(&name) {
                Fact::Unknown
            } else {
                value
            },
        );
    }
    fn assign(&mut self, target: &ast::Expr, value: Fact) {
        match target {
            ast::Expr::Name(n) => self.put(n.id.to_string(), value),
            ast::Expr::Tuple(t) => {
                for e in &t.elts {
                    self.assign(e, Fact::Unknown)
                }
            }
            ast::Expr::List(t) => {
                for e in &t.elts {
                    self.assign(e, Fact::Unknown)
                }
            }
            _ => {
                self.invalidate(&value);
                self.barrier();
            }
        }
    }
    fn invalidate(&mut self, fact: &Fact) {
        if let Fact::Tensor(t) = fact {
            for value in self.env.values_mut() {
                if let Fact::Tensor(other) = value
                    && other.id == t.id
                {
                    other.escaped = true;
                    other.device = None;
                    other.dtype = None;
                    other.previous = None;
                }
            }
        }
    }
    fn degrade_writes(&mut self, body: &[ast::Stmt]) {
        let mut uncertain = bindings::UncertainWrites::default();
        for stmt in body {
            uncertain.visit_stmt(stmt.clone());
        }
        for name in Bindings::collect(body).names {
            if uncertain.names.contains(&name) {
                self.env.insert(name, Fact::Unknown);
            } else if let Some(Fact::Tensor(t)) = self.env.get_mut(&name) {
                t.device = None;
                t.dtype = None;
                t.previous = None;
            } else {
                self.env.insert(name, Fact::Unknown);
            }
        }
    }
    fn merge(left: Env, right: Env) -> Env {
        let mut out = Env::new();
        for (name, value) in left {
            let merged = if right.get(&name) == Some(&value) {
                value
            } else if let (Fact::Tensor(a), Some(Fact::Tensor(b))) = (&value, right.get(&name)) {
                Fact::Tensor(Tensor {
                    id: a.id,
                    device: if a.device == b.device {
                        a.device.clone()
                    } else {
                        None
                    },
                    dtype: if a.dtype == b.dtype {
                        a.dtype.clone()
                    } else {
                        None
                    },
                    declared: a.declared || b.declared,
                    escaped: a.escaped || b.escaped,
                    previous: None,
                })
            } else {
                Fact::Unknown
            };
            out.insert(name, merged);
        }
        out
    }
    fn branch(&mut self, body: &[ast::Stmt], orelse: &[ast::Stmt]) {
        self.barrier();
        let before = self.env.clone();
        self.block(body);
        let left = self.env.clone();
        self.env = before;
        self.barrier();
        self.block(orelse);
        self.env = Self::merge(left, self.env.clone());
        self.barrier();
    }
    fn loop_body(
        &mut self,
        target: Option<&ast::Expr>,
        test: Option<&ast::Expr>,
        body: &[ast::Stmt],
        orelse: &[ast::Stmt],
    ) {
        self.barrier();
        let accumulations = self.accumulation_candidates(target, test, body);
        self.degrade_writes(body);
        if let Some(t) = target {
            self.assign(t, Fact::Unknown);
        }
        let entry = self.env.clone();
        self.loop_depth += 1;
        for range in accumulations {
            self.emit("M003", range, "medium", "Straight-line loop replaces an outside Tensor accumulator with cat including itself".into());
        }
        if let Some(test) = test {
            self.eval(test.clone());
        }
        self.block(body);
        self.loop_depth -= 1;
        self.env = Self::merge(entry, self.env.clone());
        self.barrier();
        self.block(orelse);
        self.barrier();
    }
    fn args(&mut self, args: &ast::Arguments) {
        let declarations = args
            .posonlyargs
            .iter()
            .chain(&args.args)
            .chain(&args.kwonlyargs)
            .map(|arg| {
                (
                    arg.def.arg.to_string(),
                    arg.def
                        .annotation
                        .as_ref()
                        .is_some_and(|e| self.resolve(e).name() == Some("torch.Tensor")),
                )
            })
            .collect::<Vec<_>>();
        for (name, declared) in declarations {
            let fact = if declared {
                self.fresh(None, None, true)
            } else {
                Fact::Unknown
            };
            self.put(name, fact);
        }
        for arg in args.vararg.iter().chain(args.kwarg.iter()) {
            self.put(arg.arg.to_string(), Fact::Unknown);
        }
    }
    fn defaults(&mut self, args: &ast::Arguments) {
        for arg in args
            .posonlyargs
            .iter()
            .chain(&args.args)
            .chain(&args.kwonlyargs)
        {
            if let Some(e) = &arg.default {
                self.eval(*e.clone());
            }
        }
    }
    fn function(
        &mut self,
        name: &str,
        args: &ast::Arguments,
        body: &[ast::Stmt],
        decorators: &[ast::Expr],
    ) {
        for e in decorators {
            self.eval(e.clone());
        }
        self.defaults(args);
        let captures = self.captured_tensors(args, body);
        self.put(name.into(), Fact::Unknown);
        let outer = self.env.clone();
        let previous_external = self.external.clone();
        let class = self.class_outer.take();
        if let Some(env) = &class {
            self.env = env.clone();
        }
        let bindings = Bindings::collect(body);
        self.external = bindings.external;
        for name in bindings.names.iter().chain(&self.external) {
            self.env.insert(name.clone(), Fact::Unknown);
        }
        self.args(args);
        let depth = self.loop_depth;
        self.loop_depth = 0;
        self.barrier();
        self.block(body);
        self.env = outer;
        self.escape_captured(captures);
        self.external = previous_external;
        self.class_outer = class;
        self.loop_depth = depth;
        self.barrier();
    }
    fn resolve(&self, e: &ast::Expr) -> Fact {
        match e {
            ast::Expr::Name(n) => self.env.get(n.id.as_str()).cloned().unwrap_or_default(),
            ast::Expr::Attribute(a) => match self.resolve(&a.value) {
                Fact::Name(n) => Fact::Name(format!("{n}.{}", a.attr)),
                _ => Fact::Unknown,
            },
            ast::Expr::Constant(c) => match &c.value {
                ast::Constant::Str(s) => Fact::String(s.clone(), c.range),
                ast::Constant::Int(n) => n
                    .to_string()
                    .parse()
                    .map(|n| Fact::Integer(n, c.range))
                    .unwrap_or_default(),
                ast::Constant::Bool(b) => Fact::Bool(*b),
                _ => Fact::Unknown,
            },
            _ => Fact::Unknown,
        }
    }
    fn comprehension(
        &mut self,
        generators: &[ast::Comprehension],
        elements: &[ast::Expr],
        deferred: bool,
    ) {
        let outer = self.env.clone();
        let depth = self.loop_depth;
        if deferred {
            self.loop_depth = 0;
        }
        self.barrier();
        let class = self.class_outer.take();
        for (index, g) in generators.iter().enumerate() {
            self.eval(g.iter.clone());
            if index == 0 {
                if let Some(env) = &class {
                    self.env = env.clone();
                }
                self.loop_depth += 1;
            }
            self.assign(&g.target, Fact::Unknown);
            for e in &g.ifs {
                self.eval(e.clone());
            }
        }
        for e in elements {
            self.eval(e.clone());
        }
        self.env = outer;
        self.class_outer = class;
        self.loop_depth = depth;
        self.barrier();
    }
    fn eval(&mut self, e: ast::Expr) -> Fact {
        if let ast::Expr::UnaryOp(_) = &e
            && let Some(n) = literal_dimension(&e)
        {
            return Fact::SignedInteger(n);
        }
        let shape = match &e {
            ast::Expr::Tuple(t) => Some(&t.elts),
            ast::Expr::List(l) => Some(&l.elts),
            _ => None,
        };
        if let Some(elements) = shape
            && let Some(dims) = elements
                .iter()
                .map(literal_dimension)
                .collect::<Option<Vec<_>>>()
        {
            return Fact::Shape(dims);
        }
        match e {
            ast::Expr::Name(_) | ast::Expr::Constant(_) => self.resolve(&e),
            ast::Expr::Attribute(a) => {
                let base = self.eval(*a.value);
                match base {
                    Fact::Name(n) => Fact::Name(format!("{n}.{}", a.attr)),
                    _ => {
                        self.barrier();
                        Fact::Unknown
                    }
                }
            }
            ast::Expr::Call(c) => self.call(c),
            ast::Expr::NamedExpr(n) => {
                let value = self.eval(*n.value);
                self.assign(&n.target, value.clone());
                value
            }
            ast::Expr::Lambda(l) => {
                self.defaults(&l.args);
                let captures = self.captured_tensors(
                    &l.args,
                    &[ast::Stmt::Expr(ast::StmtExpr {
                        range: l.range,
                        value: l.body.clone(),
                    })],
                );
                let outer = self.env.clone();
                let depth = self.loop_depth;
                self.loop_depth = 0;
                let class = self.class_outer.take();
                if let Some(class_outer) = &class {
                    self.env = class_outer.clone();
                }
                let mut bindings = Bindings::default();
                bindings.visit_expr(*l.body.clone());
                for name in bindings.names {
                    self.env.insert(name, Fact::Unknown);
                }
                self.args(&l.args);
                self.barrier();
                self.eval(*l.body);
                self.env = outer;
                self.escape_captured(captures);
                self.class_outer = class;
                self.loop_depth = depth;
                self.barrier();
                Fact::Unknown
            }
            ast::Expr::ListComp(c) => {
                self.comprehension(&c.generators, &[*c.elt], false);
                Fact::Unknown
            }
            ast::Expr::SetComp(c) => {
                self.comprehension(&c.generators, &[*c.elt], false);
                Fact::Unknown
            }
            ast::Expr::DictComp(c) => {
                self.comprehension(&c.generators, &[*c.key, *c.value], false);
                Fact::Unknown
            }
            ast::Expr::GeneratorExp(c) => {
                self.comprehension(&c.generators, &[*c.elt], true);
                Fact::Unknown
            }
            ast::Expr::BoolOp(c) => {
                let mut values = c.values.into_iter();
                if let Some(first) = values.next() {
                    self.eval(first);
                }
                for value in values {
                    let entry = self.env.clone();
                    self.barrier();
                    self.eval(value);
                    self.env = Self::merge(entry, self.env.clone());
                }
                self.barrier();
                Fact::Unknown
            }
            ast::Expr::IfExp(c) => {
                self.eval(*c.test);
                self.barrier();
                let before = self.env.clone();
                let left = self.eval(*c.body);
                let leftenv = self.env.clone();
                self.env = before;
                self.barrier();
                let right = self.eval(*c.orelse);
                self.env = Self::merge(leftenv, self.env.clone());
                self.barrier();
                if left == right { left } else { Fact::Unknown }
            }
            other => {
                self.barrier();
                self.generic_visit_expr(other);
                self.barrier();
                Fact::Unknown
            }
        }
    }
}
impl Visitor for Analyzer<'_> {
    fn visit_expr(&mut self, node: ast::Expr) {
        let fact = self.eval(node);
        self.invalidate(&fact);
    }
    fn visit_stmt(&mut self, node: ast::Stmt) {
        match node {
            ast::Stmt::Expr(s) => {
                self.eval(*s.value);
                self.barrier();
            }
            ast::Stmt::Import(s) => {
                for alias in s.names {
                    let canonical = alias.name.to_string();
                    let bound = alias
                        .asname
                        .as_ref()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| canonical.split('.').next().unwrap_or("").into());
                    let canonical = if alias.asname.is_some() {
                        canonical
                    } else {
                        canonical.split('.').next().unwrap_or("").into()
                    };
                    self.put(
                        bound,
                        if canonical == "torch"
                            || canonical.starts_with("torch.")
                            || canonical == "builtins"
                        {
                            Fact::Name(canonical)
                        } else {
                            Fact::Unknown
                        },
                    );
                }
            }
            ast::Stmt::ImportFrom(s) => {
                if s.names.iter().any(|a| a.name.as_str() == "*") {
                    for value in self.env.values_mut() {
                        *value = Fact::Unknown;
                    }
                    self.barrier();
                    return;
                }
                if let Some(module) = s.module {
                    for alias in s.names {
                        let name = alias.asname.unwrap_or(alias.name.clone()).to_string();
                        let canonical = format!("{module}.{}", alias.name);
                        self.put(
                            name,
                            if s.level.is_none_or(|v| v.to_u32() == 0)
                                && (module.as_str() == "torch"
                                    || module.starts_with("torch.")
                                    || module.as_str() == "builtins")
                                && alias.name.as_str() != "*"
                            {
                                Fact::Name(canonical)
                            } else {
                                Fact::Unknown
                            },
                        );
                    }
                }
            }
            ast::Stmt::Assign(s) => {
                let value = self.eval(*s.value);
                for target in s.targets {
                    self.assign(&target, value.clone());
                }
            }
            ast::Stmt::AnnAssign(s) => {
                let value = s.value.map(|e| self.eval(*e)).unwrap_or_default();
                self.assign(&s.target, value);
            }
            ast::Stmt::AugAssign(s) => {
                self.eval(*s.value);
                self.assign(&s.target, Fact::Unknown);
                self.barrier();
            }
            ast::Stmt::Delete(s) => {
                for e in s.targets {
                    self.assign(&e, Fact::Unknown);
                }
                self.barrier();
            }
            ast::Stmt::FunctionDef(s) => {
                self.function(s.name.as_str(), &s.args, &s.body, &s.decorator_list)
            }
            ast::Stmt::AsyncFunctionDef(s) => {
                self.function(s.name.as_str(), &s.args, &s.body, &s.decorator_list)
            }
            ast::Stmt::ClassDef(s) => {
                for e in s.decorator_list.into_iter().chain(s.bases) {
                    self.eval(e);
                }
                let outer = self.env.clone();
                let previous = self.class_outer.replace(outer.clone());
                self.barrier();
                self.block(&s.body);
                self.env = outer;
                self.class_outer = previous;
                self.put(s.name.to_string(), Fact::Unknown);
                self.barrier();
            }
            ast::Stmt::If(s) => {
                self.eval(*s.test);
                self.branch(&s.body, &s.orelse);
            }
            ast::Stmt::For(s) => {
                self.eval(*s.iter);
                self.loop_body(Some(&s.target), None, &s.body, &s.orelse);
            }
            ast::Stmt::AsyncFor(s) => {
                self.eval(*s.iter);
                self.loop_body(Some(&s.target), None, &s.body, &s.orelse);
            }
            ast::Stmt::While(s) => self.loop_body(None, Some(&s.test), &s.body, &s.orelse),
            ast::Stmt::With(s) => {
                self.barrier();
                for item in s.items {
                    self.eval(item.context_expr);
                    if let Some(e) = item.optional_vars {
                        self.assign(&e, Fact::Unknown);
                    }
                }
                self.block(&s.body);
                self.barrier();
            }
            ast::Stmt::AsyncWith(s) => {
                self.barrier();
                for item in s.items {
                    self.eval(item.context_expr);
                    if let Some(e) = item.optional_vars {
                        self.assign(&e, Fact::Unknown);
                    }
                }
                self.block(&s.body);
                self.barrier();
            }
            ast::Stmt::Try(s) => self.try_block(&s.body, &s.handlers, &s.orelse, &s.finalbody),
            ast::Stmt::TryStar(s) => self.try_block(&s.body, &s.handlers, &s.orelse, &s.finalbody),
            ast::Stmt::Match(s) => {
                self.eval(*s.subject);
                self.barrier();
                let base = self.env.clone();
                let mut merged = base.clone();
                for case in s.cases {
                    self.env = base.clone();
                    let mut bindings = Bindings::default();
                    bindings.visit_pattern(case.pattern);
                    for name in bindings.names {
                        self.put(name, Fact::Unknown);
                    }
                    if let Some(guard) = case.guard {
                        self.eval(*guard);
                    }
                    self.block(&case.body);
                    merged = Self::merge(merged, self.env.clone());
                    self.barrier();
                }
                self.env = merged;
            }
            other => self.generic_visit_stmt(other),
        }
    }
}
impl Analyzer<'_> {
    fn try_block(
        &mut self,
        body: &[ast::Stmt],
        handlers: &[ast::ExceptHandler],
        orelse: &[ast::Stmt],
        finalbody: &[ast::Stmt],
    ) {
        self.barrier();
        let base = self.env.clone();
        self.block(body);
        self.barrier();
        self.block(orelse);
        let mut merged = self.env.clone();
        for handler in handlers {
            self.env = base.clone();
            self.degrade_writes(body);
            self.barrier();
            let ast::ExceptHandler::ExceptHandler(h) = handler;
            if let Some(e) = &h.type_ {
                self.eval(*e.clone());
            }
            if let Some(name) = &h.name {
                self.put(name.to_string(), Fact::Unknown);
            }
            self.block(&h.body);
            if let Some(name) = &h.name {
                self.put(name.to_string(), Fact::Unknown);
            }
            merged = Self::merge(merged, self.env.clone());
        }
        self.env = merged;
        self.barrier();
        self.block(finalbody);
        self.barrier();
    }
}
