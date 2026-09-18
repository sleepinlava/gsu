//! A deliberately bounded syntactic recurrence check; no cross-iteration inference.
use super::*;
use ast::Ranged;

#[derive(Default)]
struct Names(BTreeSet<String>);
impl Visitor for Names {
    fn visit_expr_name(&mut self, n: ast::ExprName) {
        self.0.insert(n.id.to_string());
    }
}
fn names(e: &ast::Expr) -> BTreeSet<String> {
    let mut n = Names::default();
    n.visit_expr(e.clone());
    n.0
}
impl Analyzer<'_> {
    pub(super) fn captured_tensors(
        &self,
        args: &ast::Arguments,
        body: &[ast::Stmt],
    ) -> BTreeSet<String> {
        if !self.enabled.contains("M003") {
            return BTreeSet::new();
        }
        let bindings = Bindings::collect(body);
        let mut used = Names::default();
        for statement in body {
            used.visit_stmt(statement.clone());
        }
        used.0
            .retain(|name| !bindings.names.contains(name) || bindings.external.contains(name));
        for arg in args
            .posonlyargs
            .iter()
            .chain(&args.args)
            .chain(&args.kwonlyargs)
        {
            used.0.remove(arg.def.arg.as_str());
        }
        for arg in args.vararg.iter().chain(args.kwarg.iter()) {
            used.0.remove(arg.arg.as_str());
        }
        used.0
    }
    pub(super) fn escape_captured(&mut self, captures: BTreeSet<String>) {
        for name in captures {
            if let Some(Fact::Tensor(t)) = self.env.get_mut(&name) {
                t.escaped = true;
            }
        }
    }
    pub(super) fn accumulation_candidates(
        &self,
        target: Option<&ast::Expr>,
        test: Option<&ast::Expr>,
        body: &[ast::Stmt],
    ) -> Vec<TextRange> {
        if !self.enabled.contains("M003")
            || body.iter().any(|s| {
                !matches!(
                    s,
                    ast::Stmt::Assign(_) | ast::Stmt::Expr(_) | ast::Stmt::Pass(_)
                )
            })
        {
            return vec![];
        }
        let writes = Bindings::collect(body).names;
        let mut occurrences = BTreeMap::<String, usize>::new();
        for statement in body {
            let mut used = Names::default();
            used.visit_stmt(statement.clone());
            for name in used.0 {
                *occurrences.entry(name).or_default() += 1;
            }
        }
        let mut aliases = BTreeMap::<usize, usize>::new();
        for value in self.env.values() {
            if let Fact::Tensor(t) = value {
                *aliases.entry(t.id).or_default() += 1;
            }
        }
        let mut ranges = vec![];
        for statement in body {
            let ast::Stmt::Assign(assign) = statement else {
                continue;
            };
            let [ast::Expr::Name(acc)] = assign.targets.as_slice() else {
                continue;
            };
            let Some(Fact::Tensor(t)) = self.env.get(acc.id.as_str()) else {
                continue;
            };
            if t.escaped
                || self.external.contains(acc.id.as_str())
                || aliases.get(&t.id) != Some(&1)
            {
                continue;
            }
            if target
                .into_iter()
                .chain(test)
                .any(|e| names(e).contains(acc.id.as_str()))
            {
                continue;
            }
            let ast::Expr::Call(call) = assign.value.as_ref() else {
                continue;
            };
            if target.is_some_and(|e| !names(e).is_disjoint(&names(&call.func)))
                || self.resolve(&call.func).name() != Some("torch.cat")
                || !names(&call.func).is_disjoint(&writes)
            {
                continue;
            }
            if call.args.len() > 2
                || call.keywords.iter().any(|k| {
                    k.arg
                        .as_ref()
                        .is_none_or(|n| !["tensors", "dim"].contains(&n.as_str()))
                })
            {
                continue;
            }
            let mut keywords = BTreeMap::new();
            let mut valid = true;
            for k in &call.keywords {
                if keywords
                    .insert(k.arg.as_ref().unwrap().as_str(), &k.value)
                    .is_some()
                {
                    valid = false;
                }
            }
            if !call.args.is_empty() && keywords.contains_key("tensors")
                || call.args.len() > 1 && keywords.contains_key("dim")
            {
                valid = false;
            }
            let Some(sequence) = call
                .args
                .first()
                .or_else(|| keywords.get("tensors").copied())
            else {
                continue;
            };
            let elements = match sequence {
                ast::Expr::List(l) => &l.elts,
                ast::Expr::Tuple(t) => &t.elts,
                _ => continue,
            };
            // Direct names only: calls, unpacking and subscripts may mutate or alias the accumulator.
            if !valid
                || elements.len() < 2
                || !elements.iter().all(|e| matches!(e, ast::Expr::Name(_)))
                || elements
                    .iter()
                    .filter(|e| matches!(e, ast::Expr::Name(n) if n.id == acc.id))
                    .count()
                    != 1
            {
                continue;
            }
            if call
                .args
                .get(1)
                .or_else(|| keywords.get("dim").copied())
                .is_some_and(|e| {
                    literal_dimension(e)
                        .or_else(|| self.resolve(e).dimension())
                        .is_none()
                })
            {
                continue;
            }
            // The only two occurrences may be the assignment target and cat input.
            // Any other read/write is treated as escape, use, reset or unsupported control flow.
            if occurrences.get(acc.id.as_str()) != Some(&1) {
                continue;
            }
            ranges.push(call.range());
        }
        ranges
    }
}
