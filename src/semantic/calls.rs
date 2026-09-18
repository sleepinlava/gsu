//! Normalize supported PyTorch calls and produce conversion events.
use super::*;
impl Analyzer<'_> {
    pub(super) fn call(&mut self, c: ast::ExprCall) -> Fact {
        let (target, mut receiver, method) = if let ast::Expr::Attribute(a) = c.func.as_ref() {
            let receiver = self.eval(*a.value.clone());
            let target = match &receiver {
                Fact::Name(n) => Some(format!("{n}.{}", a.attr)),
                _ => None,
            };
            (target, receiver, a.attr.to_string())
        } else {
            (
                self.eval(*c.func.clone()).name().map(str::to_string),
                Fact::Unknown,
                String::new(),
            )
        };
        let epoch = self.epoch;
        let mut argument_epochs = vec![];
        let mut args = c
            .args
            .iter()
            .map(|e| {
                let value = self.eval(e.clone());
                argument_epochs.push(self.epoch);
                value
            })
            .collect::<Vec<_>>();
        let mut keywords = BTreeMap::new();
        let mut keyword_epochs = BTreeMap::new();
        let mut valid = !c.args.iter().any(|a| matches!(a, ast::Expr::Starred(_)));
        for kw in &c.keywords {
            let value = self.eval(kw.value.clone());
            if let Some(name) = &kw.arg {
                keyword_epochs.insert(name.to_string(), self.epoch);
                if keywords.insert(name.to_string(), value).is_some() {
                    valid = false;
                }
            } else {
                valid = false;
            }
        }
        // A later argument can call arbitrary code and mutate an earlier Tensor argument.
        let degrade = |value: &mut Fact, captured_epoch| {
            if captured_epoch != self.epoch
                && let Fact::Tensor(t) = value
            {
                t.device = None;
                t.dtype = None;
                t.previous = None;
                t.escaped = true;
            }
        };
        for (value, at) in args.iter_mut().zip(argument_epochs) {
            degrade(value, at);
        }
        for (name, value) in &mut keywords {
            degrade(value, keyword_epochs[name]);
        }
        if epoch != self.epoch
            && let Fact::Tensor(t) = &mut receiver
        {
            t.previous = None;
            t.device = None;
            t.dtype = None;
        }
        if let Some(result) = self.supported_call(
            &c,
            &supported::CallInfo {
                target: target.as_deref(),
                receiver: &receiver,
                method: &method,
                args: &args,
                keywords: &keywords,
                valid,
            },
        ) {
            return result;
        }
        if target.as_deref() == Some("torch.cuda.synchronize") {
            self.emit(
                "S002",
                c.range,
                "high",
                "Resolved torch.cuda.synchronize".into(),
            );
            self.barrier();
            return Fact::Unknown;
        }
        if target.as_deref() == Some("torch.device") {
            let first = args.first().or(keywords.get("type"));
            let mut device = first.and_then(Fact::device);
            if let Some(Fact::Integer(index, range)) = args.get(1).or(keywords.get("index")) {
                if matches!(device, Some((Device::Cuda(None), _))) {
                    device = Some((Device::Cuda(Some(*index)), Some(*range)));
                } else {
                    device = None;
                }
            } else if args.len() > 1 || keywords.contains_key("index") {
                device = None;
            }
            if !valid || args.len() > 2 {
                device = None;
            }
            self.hardcoded(&device);
            return device.map(|(d, r)| Fact::Device(d, r)).unwrap_or_default();
        }
        const FACTORIES: &[&str] = &[
            "torch.tensor",
            "torch.empty",
            "torch.zeros",
            "torch.ones",
            "torch.full",
            "torch.rand",
            "torch.randn",
            "torch.arange",
            "torch.as_tensor",
            "torch.from_numpy",
        ];
        if target.as_deref().is_some_and(|s| FACTORIES.contains(&s)) {
            self.barrier();
            let device = if valid {
                keywords.get("device").and_then(Fact::device)
            } else {
                None
            };
            let dtype = if valid {
                keywords.get("dtype").and_then(Fact::dtype)
            } else {
                None
            };
            self.hardcoded(&device);
            if dtype.as_deref() == Some("f64") {
                self.emit(
                    "P001",
                    c.range,
                    "high",
                    "Factory explicitly requests torch.float64".into(),
                );
            }
            for value in &args {
                self.invalidate(value);
            }
            return self.fresh(device.map(|(d, _)| d), dtype, false);
        }
        if let Fact::Tensor(mut tensor) = receiver.clone() {
            if epoch != self.epoch {
                tensor.previous = None;
                tensor.device = None;
                tensor.dtype = None;
            }
            if method == "item" && args.is_empty() && keywords.is_empty() && valid {
                if self.loop_depth > 0 && tensor.device != Some(Device::Cpu) {
                    self.emit(
                        "S001",
                        c.range,
                        if tensor.device.as_ref().is_some_and(Device::cuda) && !tensor.declared {
                            "high"
                        } else {
                            "medium"
                        },
                        format!("Confirmed Tensor scalar read; device {:?}", tensor.device),
                    );
                }
                self.barrier();
                return Fact::Unknown;
            }
            if ["detach", "clone"].contains(&method.as_str())
                && args.is_empty()
                && keywords.is_empty()
                && valid
            {
                if method == "clone" {
                    self.barrier();
                    self.next_id += 1;
                    tensor.id = self.next_id;
                    tensor.previous = None;
                }
                return Fact::Tensor(tensor);
            }
            if ["to", "cuda", "cpu", "float", "half", "double", "bfloat16"]
                .contains(&method.as_str())
            {
                let mut device = None;
                let mut dtype = None;
                let mut explicit_dtype = false;
                let allowed = if method == "to" {
                    &["device", "dtype", "non_blocking", "copy", "memory_format"][..]
                } else if method == "cuda" {
                    &["device", "non_blocking", "memory_format"][..]
                } else {
                    &["memory_format"][..]
                };
                if keywords.keys().any(|k| !allowed.contains(&k.as_str())) {
                    valid = false;
                }
                let mut non_blocking = false;
                if let Some(v) = keywords.get("non_blocking") {
                    if let Fact::Bool(b) = v {
                        non_blocking = *b;
                    } else {
                        valid = false;
                    }
                }
                let copy = match keywords.get("copy") {
                    None | Some(Fact::Bool(false)) => false,
                    Some(Fact::Bool(true)) => true,
                    _ => {
                        valid = false;
                        true
                    }
                };
                if method == "to" {
                    if args.len() > 2 {
                        valid = false;
                    }
                    if let Some(first) = args.first() {
                        if let Some(d) = first.device() {
                            device = Some(d);
                            if keywords.contains_key("device") {
                                valid = false;
                            }
                            if let Some(second) = args.get(1) {
                                dtype = second.dtype();
                                explicit_dtype = true;
                                if dtype.is_none() || keywords.contains_key("dtype") {
                                    valid = false;
                                }
                            }
                        } else if let Some(dt) = first.dtype() {
                            dtype = Some(dt);
                            explicit_dtype = true;
                            if args.len() > 1
                                || keywords.contains_key("dtype")
                                || keywords.contains_key("device")
                            {
                                valid = false;
                            }
                        } else if let Fact::Tensor(other) = first {
                            device = other.device.clone().map(|d| (d, None));
                            dtype = other.dtype.clone();
                            if args.len() > 1
                                || keywords.contains_key("device")
                                || keywords.contains_key("dtype")
                            {
                                valid = false;
                            }
                        } else {
                            valid = false;
                        }
                    }
                    if let Some(d) = keywords.get("device") {
                        device = d.device();
                        if device.is_none() {
                            valid = false;
                        }
                    }
                    if let Some(dt) = keywords.get("dtype") {
                        dtype = dt.dtype();
                        explicit_dtype = true;
                        if dtype.is_none() {
                            valid = false;
                        }
                    }
                } else if method == "cuda" {
                    if args.len() > 1 || (!args.is_empty() && keywords.contains_key("device")) {
                        valid = false;
                    }
                    device = match args.first().or(keywords.get("device")) {
                        None => Some((Device::Cuda(None), None)),
                        Some(Fact::Integer(i, r)) => Some((Device::Cuda(Some(*i)), Some(*r))),
                        Some(v) => v.device().filter(|(d, _)| d.cuda()),
                    };
                    if device.is_none() {
                        valid = false;
                    }
                } else {
                    if !args.is_empty() {
                        valid = false;
                    }
                    match method.as_str() {
                        "cpu" => device = Some((Device::Cpu, None)),
                        "float" => dtype = Some("f32".into()),
                        "half" => dtype = Some("f16".into()),
                        "double" => dtype = Some("f64".into()),
                        "bfloat16" => dtype = Some("bf16".into()),
                        _ => {}
                    }
                    explicit_dtype = dtype.is_some();
                }
                if !valid {
                    self.barrier();
                    tensor.device = None;
                    tensor.dtype = None;
                    tensor.previous = None;
                    return Fact::Tensor(tensor);
                }
                self.hardcoded(&device);
                let equivalent = !copy
                    && !keywords.contains_key("memory_format")
                    && (dtype.is_none() || dtype == tensor.dtype);
                let event = rules::conversion::ConversionEvent {
                    range: c.range,
                    receiver: &tensor,
                    device: device.as_ref().map(|(d, _)| d),
                    dtype: dtype.as_deref(),
                    explicit_dtype,
                    equivalent,
                    non_blocking,
                    repeated: self.loop_depth > 0,
                    epoch: self.epoch,
                    previous_line: tensor
                        .previous
                        .as_ref()
                        .map(|p| self.source.position(p.range.start().to_usize()).line),
                };
                for finding in rules::conversion::check(&event) {
                    self.emit(
                        finding.code,
                        finding.range,
                        finding.confidence,
                        finding.detail,
                    );
                }
                if let Some((dest, _)) = &device {
                    tensor.previous = if equivalent {
                        Some(Conversion {
                            before: tensor.device.clone(),
                            after: dest.clone(),
                            non_blocking,
                            range: c.range,
                            epoch: self.epoch,
                        })
                    } else {
                        None
                    };
                    tensor.device = Some(dest.clone());
                } else {
                    tensor.previous = None;
                }
                if dtype.is_some() {
                    tensor.dtype = dtype;
                }
                return Fact::Tensor(tensor);
            }
        }
        self.barrier();
        self.invalidate(&receiver);
        for a in args.iter().chain(keywords.values()) {
            self.invalidate(a);
        }
        Fact::Unknown
    }
}
