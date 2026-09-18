//! Pure conversion checks: no I/O, configuration access, or mutable semantic state.
use crate::semantic::facts::{Device, Tensor};
use rustpython_parser::text_size::TextRange;
pub struct ConversionEvent<'a> {
    pub range: TextRange,
    pub receiver: &'a Tensor,
    pub device: Option<&'a Device>,
    pub dtype: Option<&'a str>,
    pub explicit_dtype: bool,
    pub equivalent: bool,
    pub non_blocking: bool,
    pub repeated: bool,
    pub epoch: usize,
    pub previous_line: Option<usize>,
}
pub struct Finding {
    pub code: &'static str,
    pub range: TextRange,
    pub confidence: &'static str,
    pub detail: String,
}
pub fn check(e: &ConversionEvent<'_>) -> Vec<Finding> {
    let mut findings = vec![];
    let mut emit = |code, confidence, detail| {
        findings.push(Finding {
            code,
            range: e.range,
            confidence,
            detail,
        })
    };
    if e.explicit_dtype && e.dtype == Some("f64") {
        emit(
            "P001",
            "high",
            "Tensor conversion explicitly requests float64".into(),
        );
    }
    if e.repeated && e.explicit_dtype && e.dtype.is_some() && e.dtype != e.receiver.dtype.as_deref()
    {
        emit(
            "P002",
            if e.receiver.dtype.is_some() && !e.receiver.declared {
                "high"
            } else {
                "medium"
            },
            format!("dtype {:?} to {:?}", e.receiver.dtype, e.dtype),
        );
    }
    if let Some(dest) = e.device {
        let same = e.receiver.device.as_ref() == Some(dest) && dest.fixed();
        let source_cuda = e.receiver.device.as_ref().is_some_and(Device::cuda);
        let different = e.receiver.device.as_ref().is_some_and(|src| {
            src != dest && (src.cuda() != dest.cuda() || src.fixed() && dest.fixed())
        });
        if e.repeated
            && !same
            && (dest.cuda() || source_cuda)
            && (different || e.receiver.device.is_none() || !dest.fixed())
        {
            emit(
                "T001",
                if different && !e.receiver.declared {
                    "high"
                } else {
                    "medium"
                },
                format!("device {:?} to {:?}", e.receiver.device, dest),
            );
        }
        if let Some(previous) = &e.receiver.previous
            && previous.epoch == e.epoch
        {
            if &previous.after == dest
                && dest.fixed()
                && e.equivalent
                && previous.non_blocking == e.non_blocking
            {
                emit(
                    "D002",
                    "high",
                    format!(
                        "Same known device {:?}; previous conversion at line {}",
                        dest,
                        e.previous_line.unwrap_or(1)
                    ),
                );
            }
            if let Some(origin) = &previous.before
                && origin == dest
                && origin.cuda() != previous.after.cuda()
            {
                emit(
                    "T002",
                    "high",
                    format!(
                        "{:?} -> {:?} -> {:?}; sequence starts at line {}",
                        origin,
                        previous.after,
                        dest,
                        e.previous_line.unwrap_or(1)
                    ),
                );
            }
        }
    }
    findings
}
