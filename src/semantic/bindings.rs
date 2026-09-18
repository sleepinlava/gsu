use rustpython_ast::{self as ast, Visitor};
use std::collections::BTreeSet;
#[derive(Default)]
pub struct Bindings {
    pub names: BTreeSet<String>,
    pub external: BTreeSet<String>,
}
impl Bindings {
    pub fn collect(body: &[ast::Stmt]) -> Self {
        let mut b = Self::default();
        for s in body {
            b.visit_stmt(s.clone());
        }
        b
    }
}
impl Visitor for Bindings {
    fn visit_expr_name(&mut self, n: ast::ExprName) {
        if !matches!(n.ctx, ast::ExprContext::Load) {
            self.names.insert(n.id.to_string());
        }
    }
    fn visit_stmt_function_def(&mut self, n: ast::StmtFunctionDef) {
        self.names.insert(n.name.to_string());
    }
    fn visit_stmt_async_function_def(&mut self, n: ast::StmtAsyncFunctionDef) {
        self.names.insert(n.name.to_string());
    }
    fn visit_stmt_class_def(&mut self, n: ast::StmtClassDef) {
        self.names.insert(n.name.to_string());
    }
    fn visit_expr_lambda(&mut self, _: ast::ExprLambda) {}
    fn visit_expr_list_comp(&mut self, _: ast::ExprListComp) {}
    fn visit_expr_set_comp(&mut self, _: ast::ExprSetComp) {}
    fn visit_expr_dict_comp(&mut self, _: ast::ExprDictComp) {}
    fn visit_expr_generator_exp(&mut self, _: ast::ExprGeneratorExp) {}
    fn visit_stmt_import(&mut self, n: ast::StmtImport) {
        for a in n.names {
            self.names.insert(
                a.asname
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| a.name.split('.').next().unwrap_or("").to_string()),
            );
        }
    }
    fn visit_stmt_import_from(&mut self, n: ast::StmtImportFrom) {
        for a in n.names {
            self.names.insert(a.asname.unwrap_or(a.name).to_string());
        }
    }
    fn visit_stmt_global(&mut self, n: ast::StmtGlobal) {
        self.external
            .extend(n.names.into_iter().map(|s| s.to_string()));
    }
    fn visit_stmt_nonlocal(&mut self, n: ast::StmtNonlocal) {
        self.external
            .extend(n.names.into_iter().map(|s| s.to_string()));
    }
    fn visit_excepthandler_except_handler(&mut self, n: ast::ExceptHandlerExceptHandler) {
        if let Some(name) = &n.name {
            self.names.insert(name.to_string());
        }
        self.generic_visit_excepthandler_except_handler(n);
    }
    fn visit_pattern_match_as(&mut self, n: ast::PatternMatchAs) {
        if let Some(name) = &n.name {
            self.names.insert(name.to_string());
        }
        self.generic_visit_pattern_match_as(n);
    }
    fn visit_pattern_match_star(&mut self, n: ast::PatternMatchStar) {
        if let Some(name) = n.name {
            self.names.insert(name.to_string());
        }
    }
    fn visit_pattern_match_mapping(&mut self, n: ast::PatternMatchMapping) {
        if let Some(name) = &n.rest {
            self.names.insert(name.to_string());
        }
        self.generic_visit_pattern_match_mapping(n);
    }
}

/// Assignments whose result cannot retain Tensor identity across loop iterations.
#[derive(Default)]
pub struct UncertainWrites {
    pub names: BTreeSet<String>,
}
impl Visitor for UncertainWrites {
    fn visit_stmt_function_def(&mut self, _: ast::StmtFunctionDef) {}
    fn visit_stmt_async_function_def(&mut self, _: ast::StmtAsyncFunctionDef) {}
    fn visit_stmt_class_def(&mut self, _: ast::StmtClassDef) {}
    fn visit_stmt_assign(&mut self, node: ast::StmtAssign) {
        for target in &node.targets {
            if let ast::Expr::Name(name) = target {
                let retained = if let ast::Expr::Call(call) = node.value.as_ref() {
                    if let ast::Expr::Attribute(attr) = call.func.as_ref() {
                        matches!(attr.value.as_ref(), ast::Expr::Name(base) if base.id == name.id)
                            && [
                                "to", "cuda", "cpu", "float", "half", "double", "bfloat16",
                                "clone", "detach",
                            ]
                            .contains(&attr.attr.as_str())
                    } else {
                        false
                    }
                } else {
                    matches!(node.value.as_ref(), ast::Expr::Name(base) if base.id == name.id)
                };
                if !retained {
                    self.names.insert(name.id.to_string());
                }
            }
        }
        self.generic_visit_stmt_assign(node);
    }
}
