//! The ui tests

#[cfg(feature = "__ui_tests")]
#[test]
fn ui() {
    use std::path::PathBuf;

    use fs_extra::dir::CopyOptions;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct CargoMetadata {
        workspace_root: String,
    }

    let cargo_meta: CargoMetadata =
        std::process::Command::new(option_env!("CARGO").unwrap_or("cargo"))
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()
            .map(|output| serde_json::de::from_slice(&output.stdout).unwrap())
            .unwrap();

    let workspace_root = PathBuf::from(cargo_meta.workspace_root);
    let from = workspace_root
        .join("gungraun")
        .join("tests")
        .join("fixtures");
    let to = workspace_root
        .join("target")
        .join("tests")
        .join("trybuild")
        .join("gungraun")
        .join("gungraun")
        .join("tests");

    let to_fixtures = to.join("fixtures");
    if to_fixtures.exists() {
        std::fs::remove_dir_all(&to_fixtures).unwrap();
    } else {
        std::fs::create_dir_all(&to).unwrap();
    }
    fs_extra::copy_items(&[from], &to, &CopyOptions::default()).unwrap();

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/**/test_*_invalid*.rs");
    t.pass("tests/ui/**/test_*_valid*.rs");
}
