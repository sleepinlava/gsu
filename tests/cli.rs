use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};
use tempfile::TempDir;
fn run(dir: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gsu"))
        .current_dir(dir.path())
        .args(args)
        .output()
        .unwrap()
}
fn project(source: &str) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    fs::write(d.path().join("case.py"), source).unwrap();
    d
}
fn report(source: &str, select: &str) -> Value {
    let d = project(source);
    let o = run(
        &d,
        &[
            "check",
            "case.py",
            "--select",
            select,
            "--output-format",
            "json",
        ],
    );
    assert!(
        o.status.code().unwrap() < 2,
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(o.stderr.is_empty());
    serde_json::from_slice(&o.stdout).unwrap()
}
fn count(source: &str, select: &str) -> usize {
    report(source, select)["diagnostics"]
        .as_array()
        .unwrap()
        .len()
}
#[test]
fn cli_basics() {
    let d = project("");
    for args in [
        vec![],
        vec!["--help"],
        vec!["check", "--help"],
        vec!["--version"],
        vec!["rule", "T001"],
    ] {
        assert!(run(&d, &args).status.success());
    }
    assert_eq!(
        String::from_utf8(run(&d, &["--version"]).stdout).unwrap(),
        "gsu 0.2.0\n"
    );
    for args in [
        vec!["rule"],
        vec!["rule", "X001"],
        vec!["check", "--fix"],
        vec!["run"],
        vec!["check", "--select", "T", "--select", "S"],
    ] {
        let o = run(&d, &args);
        assert_eq!(o.status.code(), Some(2));
        assert!(o.stdout.is_empty());
    }
}
#[test]
fn reports_partial_failures_and_stable_output() {
    let d = project("import torch\ntorch.cuda.synchronize()\n");
    fs::write(d.path().join("broken.py"), "def :").unwrap();
    let a = run(&d, &["check", "--output-format", "json"]);
    let b = run(&d, &["check", "--output-format", "json"]);
    assert_eq!(a.stdout, b.stdout);
    assert_eq!(a.status.code(), Some(2));
    assert!(a.stderr.is_empty());
    let r: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(r["complete"], false);
    assert_eq!(r["summary"]["files_discovered"], 2);
    assert_eq!(r["summary"]["files_checked"], 1);
    assert_eq!(r["summary"]["files_failed"], 1);
    assert_eq!(r["diagnostics"][0]["rule"], "S002");
    assert_eq!(r["errors"][0]["code"], "GSU-E006");
    assert_eq!(run(&d, &["check", "--select", ""]).status.code(), Some(2));
}
#[test]
fn config_precedence_and_validation() {
    let d = project("import torch\nx=torch.ones(2,device='cuda:0',dtype=torch.float64)\n");
    fs::write(
        d.path().join("pyproject.toml"),
        "[tool.gsu]\nselect=['D','P']\nignore=['D001']\n",
    )
    .unwrap();
    let o = run(&d, &["check", "--output-format", "json"]);
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(run(&d, &["check", "--select", "D"]).status.code(), Some(0));
    assert_eq!(
        run(&d, &["check", "--select", "D", "--ignore", ""])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(run(&d, &["check", "--select", ""]).status.code(), Some(0));
    for value in [
        "[tool.gsu]\nwat=true",
        "[tool.gsu]\nselect='T'",
        "[tool.gsu]\nselect=['t']",
        "[tool.gsu]\nexclude=['[']",
        "[tool.gsu]\nexclude=['!x']",
        "not toml",
    ] {
        fs::write(d.path().join("pyproject.toml"), value).unwrap();
        let o = run(&d, &["check", "--output-format", "json"]);
        assert_eq!(o.status.code(), Some(2));
        let r: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(r["complete"], false);
        assert_eq!(r["summary"]["files_discovered"], 0);
    }
}
#[test]
fn discovery_filters_and_deduplication() {
    let d = project("");
    for sub in ["generated", "src", ".hidden", "venv", "nested"] {
        fs::create_dir(d.path().join(sub)).unwrap();
        fs::write(d.path().join(sub).join("a.py"), "").unwrap();
    }
    fs::write(d.path().join(".gitignore"), "generated/\n").unwrap();
    fs::write(d.path().join(".ignore"), "src/\n").unwrap();
    fs::write(
        d.path().join("nested/pyproject.toml"),
        "[tool.gsu]\ninvalid=1",
    )
    .unwrap();
    let o = run(
        &d,
        &[
            "check",
            ".",
            "case.py",
            "generated/a.py",
            "--exclude",
            "nested",
            "--output-format",
            "json",
        ],
    );
    assert!(o.status.success());
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["summary"]["files_checked"], 2);
    assert_eq!(r["summary"]["files_skipped"], 1);
    assert_eq!(r["summary"]["files_discovered"], 3);
}
#[test]
fn source_positions_bom_unicode_crlf() {
    let r = report(
        "\u{feff}import torch\r\n变量 = torch.ones(2); torch.cuda.synchronize()\r\n",
        "S002",
    );
    let loc = &r["diagnostics"][0]["location"];
    assert_eq!(loc["start"]["line"], 2);
    assert_eq!(loc["start"]["column"], 21);
    assert_eq!(loc["end"]["column"], 45);
}
#[test]
fn parsing_syntax_matrix() {
    for source in [
        "match x:\n case {'a': y}: pass\n",
        "try:\n pass\nexcept* ValueError:\n pass\n",
        "type Vec[T] = list[T]\n",
        "def identity[T](x: T) -> T:\n return x\n",
        "x = f\"{ {'a': 1}['a'] }\"\n",
    ] {
        assert_eq!(count(source, "S"), 0, "{source}");
    }
}
#[test]
fn safety_no_execution_or_writes() {
    let source = "from pathlib import Path\nPath('SENTINEL').write_text('bad')\nwhile True: pass\nraise RuntimeError('never execute')\n";
    let d = project(source);
    assert!(run(&d, &["check"]).status.success());
    assert!(!d.path().join("SENTINEL").exists());
    assert!(!d.path().join("__pycache__").exists());
    assert_eq!(
        fs::read_to_string(d.path().join("case.py")).unwrap(),
        source
    );
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
}
#[test]
fn encoding_size_and_depth_limits() {
    let d = project("");
    for (bytes, code) in [
        (vec![0xff], "GSU-E005"),
        (vec![b' '; 2 * 1024 * 1024 + 1], "GSU-E008"),
        (
            format!("x={}1{}", "(".repeat(257), ")".repeat(257)).into_bytes(),
            "GSU-E008",
        ),
    ] {
        fs::write(d.path().join("case.py"), bytes).unwrap();
        let o = run(&d, &["check", "--output-format", "json"]);
        assert_eq!(o.status.code(), Some(2));
        let r: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(r["errors"][0]["code"], code);
    }
}
#[test]
fn unknown_objects_never_become_tensors() {
    let src = "class Fake:\n def item(self): pass\n def cuda(self,*args): return self\n def double(self): return self\nx=Fake()\nfor _ in range(3):\n x.cuda(0).double().item()\n x.to('cuda:0').to('cpu')\n";
    assert_eq!(count(src, "T,S,D,P"), 0);
}
#[test]
fn function_shadowing_and_contexts() {
    for source in [
        "import torch\ndef f(torch):\n torch.cuda.synchronize()\n",
        "import torch\ndef f():\n torch.cuda.synchronize()\n torch=other\n",
        "from torch.cuda import synchronize as sync\ndef f():\n sync()\n sync=other\n",
        "import torch\ndel torch\ntorch.cuda.synchronize()\n",
    ] {
        assert_eq!(count(source, "S002"), 0);
    }
    assert_eq!(
        count(
            "import torch\nclass C:\n torch=other\n def f(self):\n  torch.cuda.synchronize()\n",
            "S002"
        ),
        1
    );
    assert_eq!(
        count(
            "import torch\nx=torch.ones(1,device='cpu')\nfor _ in range(2):\n def f():\n  x.cuda()\n y=lambda: x.cuda()\n",
            "T001"
        ),
        0
    );
}
#[test]
fn sequence_barriers() {
    for middle in [
        "consume(y)",
        "print(y)",
        "z=y+1",
        "if flag:\n pass",
        "for _ in []:\n pass",
    ] {
        let src = format!(
            "import torch\nx=torch.ones(1,device='cpu')\ny=x.to('cuda:0')\n{middle}\nz=y.cpu()\n"
        );
        assert_eq!(count(&src, "T002"), 0, "{middle}");
    }
}
#[test]
fn device_and_dtype_overloads() {
    let src = "import torch\nx=torch.ones(1,device='cpu',dtype=torch.float32)\nfor _ in range(2):\n y=x.to(torch.float64)\n";
    assert_eq!(count(src, "T001"), 0);
    assert_eq!(count(src, "P001,P002"), 2);
    for suffix in [
        ",copy=True",
        ",dtype=torch.float64",
        ",memory_format=torch.contiguous_format",
        ",mystery=True",
        ",non_blocking=True",
    ] {
        let src = format!("import torch\nx=torch.ones(1)\ny=x.to('cuda:0').to('cuda:0'{suffix})\n");
        assert_eq!(count(&src, "D002"), 0, "{suffix}");
    }
}
#[test]
fn consumption_and_loop_type_changes() {
    for middle in ["a=y.shape", "a=[y]", "a=(y,)", "y"] {
        let src = format!(
            "import torch\nx=torch.ones(1,device='cpu')\ny=x.cuda()\n{middle}\nz=y.cpu()\n"
        );
        assert_eq!(count(&src, "T002"), 0, "{middle}");
    }
    assert_eq!(
        count(
            "import torch\nx=torch.ones(1)\nfor i in range(2):\n x.cuda()\n x=other\n",
            "T001"
        ),
        0
    );
}
#[test]
fn invalid_selectors_and_explicit_config() {
    let d = project("");
    for token in ["T,,S", "t", "T00", "S003", ",", "T,"] {
        let o = run(&d, &["check", "--select", token, "--output-format", "json"]);
        assert_eq!(o.status.code(), Some(2));
        let r: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(r["errors"][0]["code"], "GSU-E003");
    }
    fs::write(d.path().join("other.toml"), "[unrelated]\na=1").unwrap();
    assert_eq!(
        run(&d, &["check", "--config", "other.toml"]).status.code(),
        Some(2)
    );
}
#[cfg(unix)]
#[test]
fn symbolic_links_are_never_scanned() {
    use std::os::unix::fs::symlink;
    let d = project("");
    symlink("case.py", d.path().join("linked.py")).unwrap();
    symlink(".", d.path().join("loop")).unwrap();
    let o = run(&d, &["check", "--output-format", "json"]);
    assert!(o.status.success());
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["summary"]["files_checked"], 1);
    assert_eq!(r["summary"]["files_skipped"], 1);
    assert_eq!(run(&d, &["check", "linked.py"]).status.code(), Some(2));
}
#[test]
fn terminal_control_escaping() {
    let d = project("import torch\ntorch.cuda.synchronize() # \u{1b}[31m\u{202e}bad\n");
    let o = run(&d, &["check"]);
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\u{202e}'));
    assert!(text.contains("\\u{1b}"));
}
#[test]
fn combined_selection_is_independent() {
    let src = "import torch\nx=torch.ones(1,device='cpu',dtype=torch.float32)\nfor _ in range(2):\n y=x.to('cuda:0',torch.float64)\n";
    let all = report(src, "T,S,D,P");
    for rule in ["T001", "D001", "P001", "P002"] {
        let one = report(src, rule);
        let filtered: Vec<_> = all["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["rule"] == rule)
            .collect();
        assert_eq!(
            filtered,
            one["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn while_loop_reassignment_loses_device_certainty() {
    let r = report(
        "import torch\nx=torch.ones(1,device='cpu')\nwhile flag:\n x=x.cuda()\n",
        "T001",
    );
    assert_eq!(r["diagnostics"][0]["confidence"], "medium");
}
#[test]
fn branch_and_external_bindings() {
    let src = "import torch\nif flag:\n x=torch.ones(1,device='cpu')\nelse:\n x=object()\nfor _ in range(2):\n x.cuda()\n";
    assert_eq!(count(src, "T001"), 0);
    for declaration in ["global torch", "nonlocal torch"] {
        let src = format!(
            "import torch\ndef outer():\n def inner():\n  {declaration}\n  torch.cuda.synchronize()\n  torch=other\n"
        );
        assert_eq!(count(&src, "S002"), 0);
    }
}
#[test]
fn all_factories_and_aliases() {
    for factory in [
        "tensor",
        "empty",
        "zeros",
        "ones",
        "full",
        "rand",
        "randn",
        "arange",
        "as_tensor",
        "from_numpy",
    ] {
        let src = format!(
            "from torch import {factory} as make\nx=make(data)\nfor _ in range(2):\n x.cuda()\n"
        );
        assert_eq!(count(&src, "T001"), 1, "{factory}");
    }
}
#[test]
fn long_console_line_keeps_call_visible() {
    let src = format!(
        "import torch\n{}torch.cuda.synchronize()\n",
        "x=1; ".repeat(100)
    );
    let d = project(&src);
    let o = run(&d, &["check"]);
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("…"));
    assert!(text.contains("torch.cuda.synchronize()"));
    assert!(text.lines().all(|line| line.len() < 500));
}
#[test]
fn lambda_binding_and_star_imports_are_conservative() {
    assert_eq!(
        count(
            "import torch\nf=lambda: (torch.cuda.synchronize(), (torch := other))\n",
            "S002"
        ),
        0
    );
    assert_eq!(
        count(
            "import torch\nfrom unknown import *\ntorch.cuda.synchronize()\n",
            "S002"
        ),
        0
    );
    assert_eq!(
        count(
            "import torch\nclass C:\n torch=other\n f=lambda: torch.cuda.synchronize()\n",
            "S002"
        ),
        1
    );
}
#[test]
fn real_style_review_corpus() {
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/projects/manifest.json")).unwrap();
    for sample in manifest.as_array().unwrap() {
        let source = fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/projects")
                .join(sample["file"].as_str().unwrap()),
        )
        .unwrap();
        let result = report(&source, "T,S,D,P");
        let mut actual = result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["rule"].as_str().unwrap())
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = sample["expected_rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(actual, expected, "{}: {}", sample["file"], sample["review"]);
    }
}
#[test]
fn class_comprehension_and_nested_lambda_scopes() {
    assert_eq!(
        count(
            "import torch\nclass C:\n torch=other\n values=[torch.cuda.synchronize() for i in range(2)]\n",
            "S002"
        ),
        1
    );
    assert_eq!(
        count(
            "import torch\nclass C:\n f=lambda torch: (lambda: torch.cuda.synchronize())\n",
            "S002"
        ),
        0
    );
}
#[test]
fn explicit_hidden_root_scans_visible_children() {
    let d = project("");
    fs::create_dir(d.path().join(".workspace")).unwrap();
    fs::write(d.path().join(".workspace/a.py"), "").unwrap();
    fs::create_dir(d.path().join(".workspace/.nested")).unwrap();
    fs::write(d.path().join(".workspace/.nested/b.py"), "").unwrap();
    let o = run(&d, &["check", ".workspace", "--output-format", "json"]);
    assert!(o.status.success());
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["summary"]["files_checked"], 1);
}
