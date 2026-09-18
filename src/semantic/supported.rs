//! Finite API signatures and return facts. Unsupported overloads stay unknown.
use super::*;

pub(super) struct CallInfo<'a> {
    pub target: Option<&'a str>,
    pub receiver: &'a Fact,
    pub method: &'a str,
    pub args: &'a [Fact],
    pub keywords: &'a BTreeMap<String, Fact>,
    pub valid: bool,
}

/// Reject unknown keywords, positional/keyword collisions and excess arguments.
pub(super) fn bind(
    args: &[Fact],
    keywords: &BTreeMap<String, Fact>,
    positional: &[&str],
    keyword_only: &[&str],
) -> Option<BTreeMap<String, Fact>> {
    if args.len() > positional.len()
        || keywords
            .keys()
            .any(|k| !positional.contains(&k.as_str()) && !keyword_only.contains(&k.as_str()))
    {
        return None;
    }
    let mut bound = keywords.clone();
    for (name, value) in positional.iter().zip(args) {
        if bound.insert((*name).into(), value.clone()).is_some() {
            return None;
        }
    }
    Some(bound)
}

fn tensor(value: Option<&Fact>) -> Option<&Tensor> {
    match value {
        Some(Fact::Tensor(t)) => Some(t),
        _ => None,
    }
}
fn confidence(t: &Tensor) -> &'static str {
    if !t.declared { "high" } else { "medium" }
}

impl Analyzer<'_> {
    fn host_read(
        &mut self,
        code: &'static str,
        range: rustpython_parser::text_size::TextRange,
        t: &Tensor,
    ) {
        if self.loop_depth > 0 && t.device != Some(Device::Cpu) {
            self.emit(
                code,
                range,
                if t.device.as_ref().is_some_and(Device::cuda) {
                    confidence(t)
                } else {
                    "medium"
                },
                format!("Confirmed Tensor read; device {:?}", t.device),
            );
        }
    }

    pub(super) fn supported_call(
        &mut self,
        c: &ast::ExprCall,
        info: &CallInfo<'_>,
    ) -> Option<Fact> {
        let CallInfo {
            target,
            receiver,
            method,
            args,
            keywords,
            valid,
        } = info;
        if !valid {
            return None;
        }
        let name = target.unwrap_or("");
        let recv = tensor(Some(receiver));
        if ["builtins.bool", "builtins.int", "builtins.float"].contains(&name)
            && args.len() == 1
            && keywords.is_empty()
        {
            if let Some(t) = tensor(args.first()) {
                self.host_read("S004", c.range, t);
            }
            self.barrier();
            return Some(Fact::Unknown);
        }
        if *method == "tolist"
            && let Some(t) = recv
            && args.is_empty()
            && keywords.is_empty()
        {
            self.host_read("S003", c.range, t);
            self.barrier();
            return Some(Fact::Unknown);
        }
        if ["torch.cuda.empty_cache", "torch.cuda.memory.empty_cache"].contains(&name)
            && args.is_empty()
            && keywords.is_empty()
        {
            if self.loop_depth > 0 {
                self.emit(
                    "M001",
                    c.range,
                    "high",
                    format!("Resolved {name} in repeated context"),
                );
            }
            self.barrier();
            return Some(Fact::Unknown);
        }
        if name == "torch.nonzero" || (*method == "nonzero" && recv.is_some()) {
            let bound = bind(
                args,
                keywords,
                if recv.is_some() { &[] } else { &["input"] },
                if recv.is_some() {
                    &["as_tuple"]
                } else {
                    &["as_tuple", "out"]
                },
            )?;
            let t = recv.or_else(|| tensor(bound.get("input")))?;
            if !matches!(bound.get("as_tuple"), None | Some(Fact::Bool(_))) {
                return None;
            }
            if self.loop_depth > 0 && t.device.as_ref().is_some_and(Device::cuda) {
                self.emit(
                    "S005",
                    c.range,
                    confidence(t),
                    "Confirmed CUDA Tensor passed to nonzero".into(),
                );
            }
            self.barrier();
            return Some(Fact::Unknown); // as_tuple can return a tuple; no container inference.
        }
        if name == "torch.tensor" {
            let bound = bind(
                args,
                keywords,
                &["data"],
                &["dtype", "device", "requires_grad", "pin_memory"],
            )?;
            if let Some(t) = tensor(bound.get("data"))
                && self.loop_depth > 0
            {
                self.emit(
                    "M002",
                    c.range,
                    confidence(t),
                    "torch.tensor data argument is a confirmed Tensor".into(),
                );
            }
            // Continue through the existing factory implementation.
        }
        if [
            "torch.autograd.detect_anomaly",
            "torch.autograd.set_detect_anomaly",
        ]
        .contains(&name)
        {
            let setting = name.ends_with("set_detect_anomaly");
            let bound = bind(
                args,
                keywords,
                if setting {
                    &["mode", "check_nan"]
                } else {
                    &["check_nan"]
                },
                &[],
            )?;
            if !matches!(bound.get("check_nan"), None | Some(Fact::Bool(_))) {
                return None;
            }
            if !setting || bound.get("mode") == Some(&Fact::Bool(true)) {
                self.emit(
                    "A002",
                    c.range,
                    "high",
                    format!("Resolved {name} with anomaly detection enabled"),
                );
            }
            self.barrier();
            return Some(Fact::Unknown);
        }
        let backward = *method == "backward" && recv.is_some();
        if backward || ["torch.autograd.backward", "torch.autograd.grad"].contains(&name) {
            let positional: &[&str] = if backward {
                &["gradient", "retain_graph", "create_graph", "inputs"]
            } else if name == "torch.autograd.backward" {
                &[
                    "tensors",
                    "grad_tensors",
                    "retain_graph",
                    "create_graph",
                    "grad_variables",
                    "inputs",
                ]
            } else {
                &[
                    "outputs",
                    "inputs",
                    "grad_outputs",
                    "retain_graph",
                    "create_graph",
                    "only_inputs",
                    "allow_unused",
                    "is_grads_batched",
                    "materialize_grads",
                ]
            };
            let bound = bind(args, keywords, positional, &[])?;
            let required = if backward {
                true
            } else if name == "torch.autograd.backward" {
                bound.contains_key("tensors")
            } else {
                bound.contains_key("outputs") && bound.contains_key("inputs")
            };
            if required
                && self.loop_depth > 0
                && bound.get("retain_graph") == Some(&Fact::Bool(true))
                && matches!(bound.get("create_graph"), None | Some(Fact::Bool(false)))
            {
                self.emit(
                    "A001",
                    c.range,
                    recv.map_or("high", confidence),
                    "Explicit retain_graph=True without enabled or unknown create_graph".into(),
                );
            }
            self.barrier();
            return Some(Fact::Unknown);
        }
        if [
            "torch.empty_like",
            "torch.zeros_like",
            "torch.ones_like",
            "torch.full_like",
        ]
        .contains(&name)
        {
            let bound = bind(
                args,
                keywords,
                if name == "torch.full_like" {
                    &["input", "fill_value"]
                } else {
                    &["input"]
                },
                &[
                    "dtype",
                    "layout",
                    "device",
                    "requires_grad",
                    "memory_format",
                ],
            )?;
            if name == "torch.full_like" && !bound.contains_key("fill_value") {
                return None;
            }
            let input = tensor(bound.get("input"))?;
            let device = match bound.get("device") {
                None => input.device.clone(),
                Some(v) => v.device().map(|(d, _)| d),
            };
            let dtype = match bound.get("dtype") {
                None => input.dtype.clone(),
                Some(v) => v.dtype(),
            };
            self.hardcoded(&bound.get("device").and_then(|v| v.device()));
            if bound.get("dtype").and_then(|v| v.dtype()).as_deref() == Some("f64") {
                self.emit(
                    "P001",
                    c.range,
                    "high",
                    "Like factory explicitly requests float64".into(),
                );
            }
            self.barrier();
            return Some(self.fresh(device, dtype, input.declared));
        }
        if let Some(t) = recv {
            let dimensions = |values: &[Fact]| {
                !values.is_empty()
                    && (values.iter().all(|v| v.dimension().is_some())
                        || matches!(values, [Fact::Shape(_)]))
            };
            let preserves = match *method {
                "view" | "reshape" => dimensions(args) && keywords.is_empty(),
                "flatten" => bind(args, keywords, &["start_dim", "end_dim"], &[])
                    .is_some_and(|b| b.values().all(|v| v.dimension().is_some())),
                "transpose" => bind(args, keywords, &["dim0", "dim1"], &[])
                    .is_some_and(|b| b.len() == 2 && b.values().all(|v| v.dimension().is_some())),
                "permute" => {
                    (dimensions(args) && keywords.is_empty())
                        || (args.is_empty()
                            && keywords.len() == 1
                            && matches!(keywords.get("dims"), Some(Fact::Shape(_))))
                }
                "contiguous" => bind(args, keywords, &[], &["memory_format"]).is_some_and(|b| {
                    b.get("memory_format").is_none_or(|v| {
                        matches!(
                            v.name(),
                            Some(
                                "torch.contiguous_format"
                                    | "torch.channels_last"
                                    | "torch.channels_last_3d"
                            )
                        )
                    })
                }),
                _ => false,
            };
            if preserves {
                // Views may alias; share identity so unknown mutation invalidates every view.
                // contiguous/reshape may allocate, but assuming aliasing is conservative.
                let mut result = t.clone();
                result.previous = None;
                self.barrier();
                return Some(Fact::Tensor(result));
            }
            if ["sum", "mean"].contains(method) {
                let bound = bind(args, keywords, &["dim", "keepdim"], &["dtype"])?;
                let dtype = bound.get("dtype").and_then(|v| v.dtype());
                self.barrier();
                return Some(self.fresh(t.device.clone(), dtype, t.declared));
            }
        }
        None
    }
}
