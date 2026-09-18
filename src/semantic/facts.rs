use rustpython_parser::text_size::TextRange;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Device {
    Cpu,
    Cuda(Option<u32>),
}
impl Device {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cpu" => Some(Self::Cpu),
            "cuda" => Some(Self::Cuda(None)),
            _ => s
                .strip_prefix("cuda:")
                .and_then(|i| i.parse().ok())
                .map(|i| Self::Cuda(Some(i))),
        }
    }
    pub fn fixed(&self) -> bool {
        !matches!(self, Self::Cuda(None))
    }
    pub fn cuda(&self) -> bool {
        matches!(self, Self::Cuda(_))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversion {
    pub before: Option<Device>,
    pub after: Device,
    pub non_blocking: bool,
    pub range: TextRange,
    pub epoch: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tensor {
    pub id: usize,
    pub device: Option<Device>,
    pub dtype: Option<String>,
    pub declared: bool,
    pub escaped: bool,
    pub previous: Option<Conversion>,
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Fact {
    #[default]
    Unknown,
    Name(String),
    String(String, TextRange),
    Integer(u32, TextRange),
    SignedInteger(i64),
    Shape(Vec<i64>),
    Bool(bool),
    Device(Device, Option<TextRange>),
    Tensor(Tensor),
}
impl Fact {
    pub fn dimension(&self) -> Option<i64> {
        match self {
            Self::Integer(n, _) => Some(i64::from(*n)),
            Self::SignedInteger(n) => Some(*n),
            _ => None,
        }
    }
    pub fn name(&self) -> Option<&str> {
        if let Self::Name(n) = self {
            Some(n)
        } else {
            None
        }
    }
    pub fn dtype(&self) -> Option<String> {
        let name = self.name()?.strip_prefix("torch.")?;
        Some(
            match name {
                "float64" | "double" => "f64",
                "float32" | "float" => "f32",
                "float16" | "half" => "f16",
                "bfloat16" => "bf16",
                "int64" | "long" => "i64",
                "int32" | "int" => "i32",
                "bool" => "bool",
                "uint8" => "u8",
                _ => return None,
            }
            .into(),
        )
    }
    pub fn device(&self) -> Option<(Device, Option<TextRange>)> {
        match self {
            Self::String(s, r) => Device::parse(s).map(|d| {
                let origin = matches!(d, Device::Cuda(Some(_))).then_some(*r);
                (d, origin)
            }),
            Self::Device(d, r) => Some((d.clone(), *r)),
            _ => None,
        }
    }
}
