use serde_json::Value;
use std::{fs, process::Command};
fn case(name: &str) {
    let cases: Value = serde_json::from_str(include_str!("fixtures/preview-rules.json")).unwrap();
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
            "--preview",
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
fn s003_cuda() {
    case("s003_cuda");
}
#[test]
fn s003_cpu() {
    case("s003_cpu");
}
#[test]
fn s003_unknown_object() {
    case("s003_unknown_object");
}
#[test]
fn s003_outside() {
    case("s003_outside");
}
#[test]
fn s003_annotation() {
    case("s003_annotation");
}
#[test]
fn s003_alias() {
    case("s003_alias");
}
#[test]
fn s003_comprehension() {
    case("s003_comprehension");
}
#[test]
fn s003_deferred() {
    case("s003_deferred");
}
#[test]
fn s003_nested() {
    case("s003_nested");
}
#[test]
fn s003_shadow_tensor() {
    case("s003_shadow_tensor");
}
#[test]
fn s003_while_test() {
    case("s003_while_test");
}
#[test]
fn s003_iterable() {
    case("s003_iterable");
}
#[test]
fn s004_cuda() {
    case("s004_cuda");
}
#[test]
fn s004_cpu() {
    case("s004_cpu");
}
#[test]
fn s004_unknown_object() {
    case("s004_unknown_object");
}
#[test]
fn s004_outside() {
    case("s004_outside");
}
#[test]
fn s004_annotation() {
    case("s004_annotation");
}
#[test]
fn s004_alias() {
    case("s004_alias");
}
#[test]
fn s004_comprehension() {
    case("s004_comprehension");
}
#[test]
fn s004_deferred() {
    case("s004_deferred");
}
#[test]
fn s004_nested() {
    case("s004_nested");
}
#[test]
fn s004_shadow_tensor() {
    case("s004_shadow_tensor");
}
#[test]
fn s004_while_test() {
    case("s004_while_test");
}
#[test]
fn s004_iterable() {
    case("s004_iterable");
}
#[test]
fn s005_cuda() {
    case("s005_cuda");
}
#[test]
fn s005_cpu() {
    case("s005_cpu");
}
#[test]
fn s005_unknown_object() {
    case("s005_unknown_object");
}
#[test]
fn s005_outside() {
    case("s005_outside");
}
#[test]
fn s005_annotation() {
    case("s005_annotation");
}
#[test]
fn s005_alias() {
    case("s005_alias");
}
#[test]
fn s005_comprehension() {
    case("s005_comprehension");
}
#[test]
fn s005_deferred() {
    case("s005_deferred");
}
#[test]
fn s005_nested() {
    case("s005_nested");
}
#[test]
fn s005_shadow_tensor() {
    case("s005_shadow_tensor");
}
#[test]
fn s005_while_test() {
    case("s005_while_test");
}
#[test]
fn s005_iterable() {
    case("s005_iterable");
}
#[test]
fn s003_bad_arg() {
    case("s003_bad_arg");
}
#[test]
fn s003_kwargs() {
    case("s003_kwargs");
}
#[test]
fn s003_unknown_device() {
    case("s003_unknown_device");
}
#[test]
fn s004_bool() {
    case("s004_bool");
}
#[test]
fn s004_int() {
    case("s004_int");
}
#[test]
fn s004_local_shadow() {
    case("s004_local_shadow");
}
#[test]
fn s004_parameter_shadow() {
    case("s004_parameter_shadow");
}
#[test]
fn s004_star_import() {
    case("s004_star_import");
}
#[test]
fn s004_extra_argument() {
    case("s004_extra_argument");
}
#[test]
fn s004_keyword() {
    case("s004_keyword");
}
#[test]
fn s005_functional() {
    case("s005_functional");
}
#[test]
fn s005_keyword() {
    case("s005_keyword");
}
#[test]
fn s005_alias_function() {
    case("s005_alias_function");
}
#[test]
fn s005_duplicate_input() {
    case("s005_duplicate_input");
}
#[test]
fn s005_unknown_tuple() {
    case("s005_unknown_tuple");
}
#[test]
fn s005_unpack() {
    case("s005_unpack");
}
#[test]
fn s005_method_positional() {
    case("s005_method_positional");
}
#[test]
fn m001_cuda() {
    case("m001_cuda");
}
#[test]
fn m001_memory() {
    case("m001_memory");
}
#[test]
fn m001_alias() {
    case("m001_alias");
}
#[test]
fn m001_from() {
    case("m001_from");
}
#[test]
fn m001_outside() {
    case("m001_outside");
}
#[test]
fn m001_fake() {
    case("m001_fake");
}
#[test]
fn m001_argument() {
    case("m001_argument");
}
#[test]
fn m001_kwargs() {
    case("m001_kwargs");
}
#[test]
fn m001_shadow() {
    case("m001_shadow");
}
#[test]
fn m001_deferred() {
    case("m001_deferred");
}
#[test]
fn m001_comprehension() {
    case("m001_comprehension");
}
#[test]
fn m001_reassigned() {
    case("m001_reassigned");
}
#[test]
fn m002_tensor() {
    case("m002_tensor");
}
#[test]
fn m002_cpu() {
    case("m002_cpu");
}
#[test]
fn m002_list() {
    case("m002_list");
}
#[test]
fn m002_unknown() {
    case("m002_unknown");
}
#[test]
fn m002_outside() {
    case("m002_outside");
}
#[test]
fn m002_alias() {
    case("m002_alias");
}
#[test]
fn m002_annotation() {
    case("m002_annotation");
}
#[test]
fn m002_keyword() {
    case("m002_keyword");
}
#[test]
fn m002_copy_dtype() {
    case("m002_copy_dtype");
}
#[test]
fn m002_collision() {
    case("m002_collision");
}
#[test]
fn m002_unpack() {
    case("m002_unpack");
}
#[test]
fn m002_shadow() {
    case("m002_shadow");
}
#[test]
fn m002_as_tensor() {
    case("m002_as_tensor");
}
#[test]
fn m002_fake_method() {
    case("m002_fake_method");
}
#[test]
fn m003_accumulator() {
    case("m003_accumulator");
}
#[test]
fn m003_tuple() {
    case("m003_tuple");
}
#[test]
fn m003_dim() {
    case("m003_dim");
}
#[test]
fn m003_alias() {
    case("m003_alias");
}
#[test]
fn m003_reset() {
    case("m003_reset");
}
#[test]
fn m003_escape() {
    case("m003_escape");
}
#[test]
fn m003_outside() {
    case("m003_outside");
}
#[test]
fn m003_unknown() {
    case("m003_unknown");
}
#[test]
fn m003_branch() {
    case("m003_branch");
}
#[test]
fn m003_name_alias() {
    case("m003_name_alias");
}
#[test]
fn m003_prior_escape() {
    case("m003_prior_escape");
}
#[test]
fn m003_other_assignment() {
    case("m003_other_assignment");
}
#[test]
fn m003_out() {
    case("m003_out");
}
#[test]
fn m003_namespace_shadow() {
    case("m003_namespace_shadow");
}
#[test]
fn m003_callee_reassigned() {
    case("m003_callee_reassigned");
}
#[test]
fn m003_keyword() {
    case("m003_keyword");
}
#[test]
fn m003_side_effect() {
    case("m003_side_effect");
}
#[test]
fn a001_backward() {
    case("a001_backward");
}
#[test]
fn a001_annotation() {
    case("a001_annotation");
}
#[test]
fn a001_outside() {
    case("a001_outside");
}
#[test]
fn a001_false() {
    case("a001_false");
}
#[test]
fn a001_dynamic() {
    case("a001_dynamic");
}
#[test]
fn a001_higher_order() {
    case("a001_higher_order");
}
#[test]
fn a001_unknown_create() {
    case("a001_unknown_create");
}
#[test]
fn a001_explicit_false() {
    case("a001_explicit_false");
}
#[test]
fn a001_functional() {
    case("a001_functional");
}
#[test]
fn a001_grad() {
    case("a001_grad");
}
#[test]
fn a001_positional() {
    case("a001_positional");
}
#[test]
fn a001_kwargs() {
    case("a001_kwargs");
}
#[test]
fn a001_fake() {
    case("a001_fake");
}
#[test]
fn a001_collision() {
    case("a001_collision");
}
#[test]
fn a001_missing_inputs() {
    case("a001_missing_inputs");
}
#[test]
fn a001_shadow() {
    case("a001_shadow");
}
#[test]
fn a002_context() {
    case("a002_context");
}
#[test]
fn a002_direct() {
    case("a002_direct");
}
#[test]
fn a002_setting() {
    case("a002_setting");
}
#[test]
fn a002_keyword() {
    case("a002_keyword");
}
#[test]
fn a002_disabled() {
    case("a002_disabled");
}
#[test]
fn a002_dynamic() {
    case("a002_dynamic");
}
#[test]
fn a002_alias() {
    case("a002_alias");
}
#[test]
fn a002_check_nan_false() {
    case("a002_check_nan_false");
}
#[test]
fn a002_unpack() {
    case("a002_unpack");
}
#[test]
fn a002_shadow() {
    case("a002_shadow");
}
#[test]
fn a002_fake() {
    case("a002_fake");
}
#[test]
fn a002_bad_argument() {
    case("a002_bad_argument");
}
#[test]
fn a002_nested_once() {
    case("a002_nested_once");
}
#[test]
fn a002_collision() {
    case("a002_collision");
}
