use serde_json::Value;
use std::{fs, process::Command};
fn case(name: &str) {
    let cases: Value = serde_json::from_str(include_str!("fixtures/rules.json")).unwrap();
    let c = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            format!(
                "{}_{}",
                c["rule"].as_str().unwrap().to_lowercase(),
                c["name"].as_str().unwrap()
            ) == name
        })
        .unwrap();
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("case.py"), c["source"].as_str().unwrap()).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_gsu"))
        .current_dir(d.path())
        .args([
            "check",
            "case.py",
            "--select",
            c["rule"].as_str().unwrap(),
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        o.status.code().unwrap() < 2,
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    let ds = r["diagnostics"].as_array().unwrap();
    assert_eq!(ds.len() as u64, c["count"].as_u64().unwrap(), "{name}: {r}");
    for (index, diag) in ds.iter().enumerate() {
        assert_eq!(
            diag["location"], c["locations"][index],
            "{name}: precise source range"
        );
        assert_eq!(diag["rule"], c["rule"]);
        assert_eq!(diag["confidence"], c["confidence"], "{name}");
        assert_eq!(diag["severity"], c["severity"], "{name}");
        let loc = &diag["location"];
        assert_eq!(loc["path"], "case.py");
        assert!(loc["start"]["line"].as_u64().unwrap() > 0);
        assert!(!diag["evidence"].as_array().unwrap().is_empty());
    }
}
#[test]
fn t001_cpu_cuda() {
    case("t001_cpu_cuda");
}
#[test]
fn t001_cuda_cpu() {
    case("t001_cuda_cpu");
}
#[test]
fn t001_cross_card() {
    case("t001_cross_card");
}
#[test]
fn t001_same_cpu() {
    case("t001_same_cpu");
}
#[test]
fn t001_same_card() {
    case("t001_same_card");
}
#[test]
fn t001_dtype_only() {
    case("t001_dtype_only");
}
#[test]
fn t001_nonblocking() {
    case("t001_nonblocking");
}
#[test]
fn t001_annotation() {
    case("t001_annotation");
}
#[test]
fn t001_unknown() {
    case("t001_unknown");
}
#[test]
fn t001_while_condition() {
    case("t001_while_condition");
}
#[test]
fn t001_iterable() {
    case("t001_iterable");
}
#[test]
fn t001_outside() {
    case("t001_outside");
}
#[test]
fn t002_chain() {
    case("t002_chain");
}
#[test]
fn t002_reverse() {
    case("t002_reverse");
}
#[test]
fn t002_assignment() {
    case("t002_assignment");
}
#[test]
fn t002_alias() {
    case("t002_alias");
}
#[test]
fn t002_oneway() {
    case("t002_oneway");
}
#[test]
fn t002_other_tensor() {
    case("t002_other_tensor");
}
#[test]
fn t002_reassign() {
    case("t002_reassign");
}
#[test]
fn t002_consume() {
    case("t002_consume");
}
#[test]
fn t002_branch() {
    case("t002_branch");
}
#[test]
fn t002_different_index() {
    case("t002_different_index");
}
#[test]
fn t002_unknown_start() {
    case("t002_unknown_start");
}
#[test]
fn t002_three_casts() {
    case("t002_three_casts");
}
#[test]
fn s001_cuda() {
    case("s001_cuda");
}
#[test]
fn s001_cpu() {
    case("s001_cpu");
}
#[test]
fn s001_unknown_device() {
    case("s001_unknown_device");
}
#[test]
fn s001_annotation() {
    case("s001_annotation");
}
#[test]
fn s001_fake() {
    case("s001_fake");
}
#[test]
fn s001_nested() {
    case("s001_nested");
}
#[test]
fn s001_conditional() {
    case("s001_conditional");
}
#[test]
fn s001_outside() {
    case("s001_outside");
}
#[test]
fn s001_lambda() {
    case("s001_lambda");
}
#[test]
fn s001_arguments() {
    case("s001_arguments");
}
#[test]
fn s001_keyword() {
    case("s001_keyword");
}
#[test]
fn s001_comprehension() {
    case("s001_comprehension");
}
#[test]
fn s002_full() {
    case("s002_full");
}
#[test]
fn s002_alias() {
    case("s002_alias");
}
#[test]
fn s002_from() {
    case("s002_from");
}
#[test]
fn s002_device() {
    case("s002_device");
}
#[test]
fn s002_loop() {
    case("s002_loop");
}
#[test]
fn s002_shadow() {
    case("s002_shadow");
}
#[test]
fn s002_local() {
    case("s002_local");
}
#[test]
fn s002_event() {
    case("s002_event");
}
#[test]
fn s002_timing() {
    case("s002_timing");
}
#[test]
fn s002_string() {
    case("s002_string");
}
#[test]
fn s002_delete() {
    case("s002_delete");
}
#[test]
fn s002_rebind() {
    case("s002_rebind");
}
#[test]
fn d001_string() {
    case("d001_string");
}
#[test]
fn d001_two_args() {
    case("d001_two_args");
}
#[test]
fn d001_cuda_method() {
    case("d001_cuda_method");
}
#[test]
fn d001_factory() {
    case("d001_factory");
}
#[test]
fn d001_constant() {
    case("d001_constant");
}
#[test]
fn d001_plain_string() {
    case("d001_plain_string");
}
#[test]
fn d001_dynamic() {
    case("d001_dynamic");
}
#[test]
fn d001_negative() {
    case("d001_negative");
}
#[test]
fn d001_cpu() {
    case("d001_cpu");
}
#[test]
fn d001_multidigit() {
    case("d001_multidigit");
}
#[test]
fn d001_shadow() {
    case("d001_shadow");
}
#[test]
fn d001_unspecified() {
    case("d001_unspecified");
}
#[test]
fn d002_cuda() {
    case("d002_cuda");
}
#[test]
fn d002_cpu() {
    case("d002_cpu");
}
#[test]
fn d002_different() {
    case("d002_different");
}
#[test]
fn d002_copy() {
    case("d002_copy");
}
#[test]
fn d002_dtype() {
    case("d002_dtype");
}
#[test]
fn d002_memory() {
    case("d002_memory");
}
#[test]
fn d002_assignment() {
    case("d002_assignment");
}
#[test]
fn d002_call() {
    case("d002_call");
}
#[test]
fn d002_fake() {
    case("d002_fake");
}
#[test]
fn d002_current() {
    case("d002_current");
}
#[test]
fn d002_normalized() {
    case("d002_normalized");
}
#[test]
fn d002_factory_once() {
    case("d002_factory_once");
}
#[test]
fn p001_float64() {
    case("p001_float64");
}
#[test]
fn p001_double_constant() {
    case("p001_double_constant");
}
#[test]
fn p001_method() {
    case("p001_method");
}
#[test]
fn p001_to_pos() {
    case("p001_to_pos");
}
#[test]
fn p001_to_kw() {
    case("p001_to_kw");
}
#[test]
fn p001_alias() {
    case("p001_alias");
}
#[test]
fn p001_local() {
    case("p001_local");
}
#[test]
fn p001_python() {
    case("p001_python");
}
#[test]
fn p001_numpy() {
    case("p001_numpy");
}
#[test]
fn p001_unknown() {
    case("p001_unknown");
}
#[test]
fn p001_cpu() {
    case("p001_cpu");
}
#[test]
fn p001_implicit() {
    case("p001_implicit");
}
#[test]
fn p002_float() {
    case("p002_float");
}
#[test]
fn p002_half() {
    case("p002_half");
}
#[test]
fn p002_double() {
    case("p002_double");
}
#[test]
fn p002_bfloat16() {
    case("p002_bfloat16");
}
#[test]
fn p002_to() {
    case("p002_to");
}
#[test]
fn p002_same() {
    case("p002_same");
}
#[test]
fn p002_different() {
    case("p002_different");
}
#[test]
fn p002_device_only() {
    case("p002_device_only");
}
#[test]
fn p002_dynamic() {
    case("p002_dynamic");
}
#[test]
fn p002_unknown() {
    case("p002_unknown");
}
#[test]
fn p002_outside() {
    case("p002_outside");
}
#[test]
fn p002_copy_same() {
    case("p002_copy_same");
}
