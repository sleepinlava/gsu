use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};
const SOURCE: &str =
    "import torch\nx = torch.ones(2, device='cuda')\nfor i in range(3):\n    x.tolist()\n";
fn project(source: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("case.py"), source).unwrap();
    dir
}
fn run(dir: &tempfile::TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gsu"))
        .current_dir(dir.path())
        .env_remove("NO_COLOR")
        .args(args)
        .output()
        .unwrap()
}
fn report(source: &str, selected: &str) -> Value {
    let dir = project(source);
    let result = run(
        &dir,
        &[
            "check",
            "--preview",
            "--select",
            selected,
            "--output-format",
            "json",
        ],
    );
    assert!(
        result.status.code().unwrap() < 2,
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn count(source: &str, selected: &str) -> usize {
    report(source, selected)["diagnostics"]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn preview_defaults_selection_and_errors() {
    let dir = project(SOURCE);
    for args in [vec!["check"], vec!["check", "--select", "S,M,A"]] {
        assert_eq!(run(&dir, &args).status.code(), Some(0));
    }
    assert_eq!(
        run(&dir, &["check", "--select", "S003"]).status.code(),
        Some(2)
    );
    for args in [
        vec!["check", "--preview"],
        vec!["check", "--preview", "--select", "S"],
    ] {
        assert_eq!(run(&dir, &args).status.code(), Some(1));
    }
    for args in [
        vec!["check", "--preview", "--select", "M"],
        vec!["check", "--preview", "--ignore", "S003"],
        vec!["check", "--ignore", "S003"],
        vec!["check", "--preview", "--select", ""],
    ] {
        assert_eq!(run(&dir, &args).status.code(), Some(0));
    }
    let o = run(
        &dir,
        &["check", "--select", "S003", "--output-format", "json"],
    );
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["schema_version"], "2");
    assert_eq!(r["complete"], false);
    assert!(
        r["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("--preview")
    );
    assert_eq!(
        run(&dir, &["check", "--preview", "--no-preview"])
            .status
            .code(),
        Some(2)
    );
}
#[test]
fn preview_configuration_override() {
    let dir = project(SOURCE);
    fs::write(
        dir.path().join("pyproject.toml"),
        "[tool.gsu]\npreview=true\nselect=['S']\n",
    )
    .unwrap();
    assert_eq!(run(&dir, &["check"]).status.code(), Some(1));
    assert_eq!(run(&dir, &["check", "--no-preview"]).status.code(), Some(0));
    assert_eq!(
        run(&dir, &["check", "--select", "M"]).status.code(),
        Some(0)
    );
    fs::write(
        dir.path().join("pyproject.toml"),
        "[tool.gsu]\npreview=false\nignore=['S003']\n",
    )
    .unwrap();
    assert_eq!(run(&dir, &["check", "--preview"]).status.code(), Some(0));
    assert_eq!(
        run(&dir, &["check", "--preview", "--ignore", ""])
            .status
            .code(),
        Some(1)
    );
    fs::write(
        dir.path().join("pyproject.toml"),
        "[tool.gsu]\npreview='true'\n",
    )
    .unwrap();
    assert_eq!(run(&dir, &["check"]).status.code(), Some(2));
}
#[test]
fn report_snapshots_and_console_alias() {
    let dir = project(SOURCE);
    for format in ["full", "console"] {
        let o = run(
            &dir,
            &[
                "check",
                "--preview",
                "--select",
                "S003",
                "--output-format",
                format,
            ],
        );
        assert_eq!(
            String::from_utf8(o.stdout).unwrap(),
            include_str!("snapshots/full.txt")
        );
    }
    let default = run(&dir, &["check", "--preview", "--select", "S003"]);
    assert_eq!(
        String::from_utf8(default.stdout).unwrap(),
        include_str!("snapshots/full.txt")
    );
    let concise = run(
        &dir,
        &[
            "check",
            "--preview",
            "--select",
            "S003",
            "--output-format",
            "concise",
        ],
    );
    assert_eq!(
        String::from_utf8(concise.stdout).unwrap(),
        include_str!("snapshots/concise.txt")
    );
}
#[test]
fn color_overrides_and_json_stays_plain() {
    let dir = project(SOURCE);
    for color in ["auto", "never", "always"] {
        let o = run(&dir, &["check", "--preview", "--color", color]);
        assert_eq!(o.stdout.contains(&27), color == "always");
        let o = run(
            &dir,
            &[
                "check",
                "--preview",
                "--color",
                color,
                "--output-format",
                "json",
            ],
        );
        assert!(!o.stdout.contains(&27));
        assert_eq!(
            serde_json::from_slice::<Value>(&o.stdout).unwrap()["schema_version"],
            "2"
        );
    }
    let o = Command::new(env!("CARGO_BIN_EXE_gsu"))
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .args(["check", "--preview"])
        .output()
        .unwrap();
    assert!(!o.stdout.contains(&27));
}
#[test]
fn multiline_unicode_and_tab_reports() {
    let source = "\u{feff}import torch\r\n张量 = torch.ones(2, device='cuda')\r\nfor i in range(2):\r\n\t张量.tolist(\r\n\t)\r\n";
    let dir = project(source);
    let o = run(&dir, &["check", "--preview", "--select", "S003"]);
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("case.py:4:2"), "{text}");
    assert!(text.contains("continues to 5:3"), "{text}");
    assert!(text.contains("    张量.tolist("), "{text}");
    assert!(!text.contains('\t'));
    assert_eq!(
        report(source, "S003")["diagnostics"][0]["location"]["start"]["column"],
        2
    );
}
#[test]
fn preview_partial_failure_and_stable_output() {
    let dir = project(SOURCE);
    fs::write(dir.path().join("broken.py"), "def :").unwrap();
    for format in ["full", "concise", "json"] {
        let args = ["check", "--preview", "--output-format", format];
        let a = run(&dir, &args);
        let b = run(&dir, &args);
        assert_eq!(a.status.code(), Some(2));
        assert_eq!(a.stdout, b.stdout);
        if format == "json" {
            assert!(a.stderr.is_empty());
        } else {
            assert!(String::from_utf8(a.stderr).unwrap().contains("GSU-E006"));
        }
    }
}
#[test]
fn rule_help_exposes_status_and_limits() {
    let dir = project("");
    for rule in [
        "S003", "S004", "S005", "M001", "M002", "M003", "A001", "A002",
    ] {
        let o = run(&dir, &["rule", rule]);
        let text = String::from_utf8(o.stdout).unwrap();
        assert!(o.status.success());
        assert!(text.contains("Status: preview"));
        assert!(text.contains("Boundaries"));
    }
    assert!(
        String::from_utf8(run(&dir, &["rule", "T001"]).stdout)
            .unwrap()
            .contains("Status: stable")
    );
}
#[test]
fn like_factories_propagate_and_override_facts() {
    for factory in ["empty_like", "zeros_like", "ones_like", "full_like"] {
        let fill = if factory == "full_like" { ", 1" } else { "" };
        let source = format!(
            "import torch\nx = torch.ones(2, device='cuda', dtype=torch.float32)\ny = torch.{factory}(x{fill})\nfor i in range(2):\n    y.tolist()\n    y.float()\n"
        );
        assert_eq!(count(&source, "S003"), 1, "{factory}");
        assert_eq!(count(&source, "P002"), 0, "{factory}");
        let cpu = source.replace(&format!("(x{fill})"), &format!("(x{fill}, device='cpu')"));
        assert_eq!(count(&cpu, "S003"), 0, "{factory}");
        let unknown = source.replace(&format!("(x{fill})"), &format!("(x{fill}, device=device)"));
        assert_eq!(
            report(&unknown, "S003")["diagnostics"][0]["confidence"],
            "medium"
        );
    }
    let src = "import torch\nx=torch.ones(2)\ny=torch.zeros_like(x, device='cuda:1', dtype=torch.float64)\n";
    assert_eq!(count(src, "D001,P001"), 2);
}
#[test]
fn like_factory_overloads_do_not_guess() {
    for expression in [
        "torch.zeros_like(obj)",
        "torch.zeros_like(x, x)",
        "torch.zeros_like(x, input=x)",
        "torch.zeros_like(x, **opts)",
        "torch.zeros_like(x, bogus=True)",
        "torch.full_like(x)",
    ] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cuda')\ny={expression}\nfor i in range(2):\n    y.tolist()\n"
        );
        assert_eq!(count(&source, "S003"), 0, "{expression}");
    }
}
#[test]
fn shape_operators_keep_device_and_dtype() {
    for operation in [
        "view(-1)",
        "view((2,))",
        "reshape(2)",
        "reshape((2,))",
        "flatten()",
        "flatten(start_dim=0, end_dim=-1)",
        "transpose(0, 1)",
        "permute(1, 0)",
        "permute(dims=(1, 0))",
        "contiguous()",
        "contiguous(memory_format=torch.contiguous_format)",
    ] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cuda',dtype=torch.float32)\ny=x.{operation}\nfor i in range(2):\n    y.tolist()\n    y.float()\n"
        );
        let r = report(&source, "S003,P002");
        assert_eq!(
            r["diagnostics"].as_array().unwrap().len(),
            1,
            "{operation}: {r}"
        );
        assert_eq!(r["diagnostics"][0]["rule"], "S003");
        assert_eq!(r["diagnostics"][0]["confidence"], "high", "{operation}");
    }
}
#[test]
fn unsupported_shape_overloads_lose_facts() {
    for operation in [
        "view(torch.int32)",
        "view(dtype)",
        "view()",
        "reshape()",
        "transpose(0)",
        "flatten(bogus=1)",
        "permute(**opts)",
        "contiguous(memory_format=fmt)",
    ] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cuda')\ny=x.{operation}\nfor i in range(2):\n    y.tolist()\n"
        );
        assert_eq!(count(&source, "S003"), 0, "{operation}");
    }
}
#[test]
fn reductions_keep_device_without_guessing_dtype() {
    for operation in ["sum()", "mean()", "sum(dim=0, keepdim=True)", "mean(0)"] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cuda')\nfor i in range(2):\n    y=x.{operation}\n    y.item()\n"
        );
        let r = report(&source, "S001");
        assert_eq!(r["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(r["diagnostics"][0]["confidence"], "high");
    }
    for (reduction, expected) in [
        ("sum(dtype=torch.float32)", 0),
        ("sum()", 1),
        ("mean(dtype=dtype)", 1),
    ] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cuda',dtype=torch.float32)\nfor i in range(2):\n    y=x.{reduction}\n    y.float()\n"
        );
        assert_eq!(count(&source, "P002"), expected, "{reduction}");
    }
}
#[test]
fn operator_results_break_conversion_sequences() {
    for operation in [
        "view(2)",
        "reshape(2)",
        "flatten()",
        "contiguous()",
        "sum()",
        "mean()",
    ] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cpu')\ny=x.to('cuda:0').{operation}.to('cuda:0')\nz=x.cuda().{operation}.cpu()\n"
        );
        assert_eq!(count(&source, "T002,D002"), 0, "{operation}");
    }
    let source = "import torch\nx=torch.ones(2,device='cpu')\ny=x.to('cuda:0')\nz=torch.ones_like(y)\ny.to('cuda:0')\n";
    assert_eq!(count(source, "D002"), 0);
}
#[test]
fn view_aliases_and_later_arguments_invalidate_facts() {
    let source = "import torch\nx=torch.ones(2,device='cuda')\ny=x.view(2)\nmutate(y)\nfor i in range(2):\n    x.nonzero()\n";
    assert_eq!(count(source, "S005"), 0);
    let source = "import torch\nx=torch.ones(2,device='cuda')\ny=torch.ones_like(x,dtype=mutate(x))\nfor i in range(2):\n    y.nonzero()\n";
    assert_eq!(count(source, "S005"), 0);
    let source = "import torch\nx=torch.ones(2,device='cuda')\nfor i in range(2):\n    torch.nonzero(x, out=mutate(x))\n";
    assert_eq!(count(source, "S005"), 0);
}
#[test]
fn builtin_alias_and_control_flow_shadowing() {
    let source = "import torch\nx=torch.ones(2,device='cuda')\nconvert=float\nfor i in range(2):\n    convert(x)\n";
    assert_eq!(count(source, "S004"), 1);
    for shadow in ["if flag:\n    float=custom", "del float", "float = custom"] {
        let source = format!(
            "import torch\nx=torch.ones(2,device='cuda')\n{shadow}\nfor i in range(2):\n    float(x)\n"
        );
        assert_eq!(count(&source, "S004"), 0);
    }
}

#[test]
fn tensor_stored_in_attribute_is_an_escape() {
    let source = "import torch\nacc=torch.empty(0)\npart=torch.ones(2)\nobj.saved=acc\nfor i in range(3):\n    acc=torch.cat([acc,part])\n";
    assert_eq!(count(source, "M003"), 0);
}
#[test]
fn explicit_builtin_imports_and_shadowing() {
    for (import, call) in [
        ("import builtins as b", "b.float(x)"),
        ("from builtins import int as convert", "convert(x)"),
    ] {
        let source = format!(
            "import torch\n{import}\nx=torch.ones(2,device='cuda')\nfor i in range(3):\n    {call}\n"
        );
        assert_eq!(count(&source, "S004"), 1);
    }
}
#[test]
fn colored_full_snapshot_and_concise_snapshot() {
    let dir = project(SOURCE);
    for (format, plain) in [
        ("full", include_str!("snapshots/full.txt")),
        ("concise", include_str!("snapshots/concise.txt")),
    ] {
        let output = run(
            &dir,
            &[
                "check",
                "--preview",
                "--select",
                "S003",
                "--color",
                "always",
                "--output-format",
                format,
            ],
        );
        let expected = plain
            .replacen("S003", "\x1b[1;33mS003\x1b[0m", 1)
            .replace("    ^^^^^^^^^^", "\x1b[33m    ^^^^^^^^^^\x1b[0m");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn preview_review_projects() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview-projects");
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/preview-projects/manifest.json")).unwrap();
    for sample in manifest.as_array().unwrap() {
        let source = fs::read_to_string(root.join(sample["file"].as_str().unwrap())).unwrap();
        let result = report(&source, "S003,S004,S005,M,A");
        let actual = result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["rule"].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            &actual,
            sample["expected_rules"].as_array().unwrap(),
            "{}: {}",
            sample["file"],
            sample["review"]
        );
        let default = run(
            &project(&source),
            &["check", "--select", "S003", "--output-format", "json"],
        );
        assert_eq!(default.status.code(), Some(2));
    }
}

#[test]
fn accumulator_captured_by_callback_is_excluded() {
    for capture in [
        "def callback():\n    return acc\n",
        "callback = lambda: acc\n",
        "def callback():\n    global acc\n    acc = replacement\n",
    ] {
        let source = format!(
            "import torch\nacc=torch.empty(0)\npart=torch.ones(2)\n{capture}for i in range(3):\n    acc=torch.cat([acc,part])\n"
        );
        assert_eq!(count(&source, "M003"), 0, "{capture}");
    }
    let source = "import torch\nacc=torch.empty(0)\npart=torch.ones(2)\ndef callback(acc):\n    return acc\nfor i in range(3):\n    acc=torch.cat([acc,part],dim=-1)\n";
    assert_eq!(count(source, "M003"), 1);
}
