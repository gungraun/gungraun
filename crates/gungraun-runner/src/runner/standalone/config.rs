//! Versioned user-facing configuration for standalone binary benchmarks.
//!
//! The TOML schema deliberately does not deserialize the private harness protocol. Its small
//! configuration model is validated and then translated to the same binary benchmark types.

// spell-checker: ignore nprogramm programm

use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::api::{
    BinaryBenchmark, BinaryBenchmarkBench, BinaryBenchmarkConfig, BinaryBenchmarkGroup,
    BinaryBenchmarkGroups, CallgrindRegressionConfig, Command, CommandKind, EntryPoint, EventKind,
    ExitWith, Limit, Sandbox, Tool, ToolRegressionConfig, ToolSpec, ToolSpecs,
};
use crate::error::Error;

pub(super) const GROUP_ID: &str = "cases";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Callgrind {
    entry_point: Option<String>,
    #[serde(default)]
    hard_limits: HardLimits,
}

/// A case duplicates the settings fields to reject unknown keys at every table level. Combining
/// `serde(flatten)` and `deny_unknown_fields` would not provide that guarantee.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    args: Option<Vec<String>>,
    #[serde(default)]
    callgrind: Callgrind,
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    expected_exit: Option<i32>,
    id: String,
    program: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    cases: Vec<Case>,
    #[serde(default)]
    defaults: Settings,
    output_dir: Option<String>,
    schema_version: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct HardLimits {
    #[serde(rename = "Ir")]
    ir: Option<u64>,
}

/// Validated standalone configuration and its equivalent runner input.
#[derive(Debug)]
pub(super) struct Manifest {
    pub case_ids: Vec<String>,
    pub groups: BinaryBenchmarkGroups,
    pub output_dir: PathBuf,
    pub root: PathBuf,
    pub source: PathBuf,
    pub uncapped: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    args: Option<Vec<String>>,
    #[serde(default)]
    callgrind: Callgrind,
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    expected_exit: Option<i32>,
    program: Option<String>,
}

impl Case {
    fn into_settings(self) -> (String, Settings) {
        (
            self.id,
            Settings {
                args: self.args,
                callgrind: self.callgrind,
                cwd: self.cwd,
                env: self.env,
                expected_exit: self.expected_exit,
                program: self.program,
            },
        )
    }
}

impl Settings {
    fn inherit(mut self, defaults: &Self) -> Self {
        self.program = self.program.or_else(|| defaults.program.clone());
        self.args = self.args.or_else(|| defaults.args.clone());
        self.cwd = self.cwd.or_else(|| defaults.cwd.clone());
        self.expected_exit = self.expected_exit.or(defaults.expected_exit);

        let mut env = defaults.env.clone();
        env.extend(self.env);
        self.env = env;

        // Merge nested fields independently: a per-case cap must preserve an inherited boundary.
        self.callgrind.entry_point = self
            .callgrind
            .entry_point
            .or_else(|| defaults.callgrind.entry_point.clone());
        self.callgrind.hard_limits.ir = self
            .callgrind
            .hard_limits
            .ir
            .or(defaults.callgrind.hard_limits.ir);
        self
    }

    fn validate(&self, source: &Path, scope: &str) -> Result<(), Error> {
        for (name, value) in [
            ("program", self.program.as_deref()),
            ("cwd", self.cwd.as_deref()),
            (
                "callgrind.entry_point",
                self.callgrind.entry_point.as_deref(),
            ),
        ] {
            if let Some(value) = value {
                validate_nonempty(source, &format!("{scope}.{name}"), value)?;
            }
        }
        if self
            .expected_exit
            .is_some_and(|value| !(0..=255).contains(&value))
        {
            return Err(manifest_error(
                source,
                format!("{scope}.expected_exit must be an integer between 0 and 255"),
            ));
        }
        if self.callgrind.hard_limits.ir == Some(0) {
            return Err(manifest_error(
                source,
                format!("{scope}.callgrind.hard_limits.Ir must be greater than zero"),
            ));
        }
        for arg in self.args.iter().flatten() {
            if arg.contains('\0') {
                return Err(manifest_error(
                    source,
                    format!("{scope}.args must not contain NUL bytes"),
                ));
            }
        }
        for (key, value) in &self.env {
            if key.is_empty() || key.contains(['=', '\0']) || value.contains('\0') {
                return Err(manifest_error(
                    source,
                    format!(
                        "{scope}.env has an invalid environment entry: keys must be nonempty and \
                         contain neither '=' nor NUL; values must not contain NUL"
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// Read a manifest, check all declared cases, and build the existing binary benchmark model.
///
/// Explicit paths are resolved relative to the configuration file, independently of the case's
/// working directory. Bare program names use the case's declared PATH, or the invocation's PATH.
/// Relative PATH entries are resolved relative to the configuration file as well.
pub(super) fn load(path: &Path, strict: bool) -> Result<Manifest, Error> {
    let source = fs::canonicalize(path)
        .map_err(|error| manifest_error(path, format!("cannot resolve configuration: {error}")))?;
    let root = source
        .parent()
        .ok_or_else(|| manifest_error(&source, "configuration has no parent directory"))?
        .to_owned();
    let contents = fs::read_to_string(&source)
        .map_err(|error| manifest_error(&source, format!("cannot read configuration: {error}")))?;
    let document: Document =
        toml::from_str(&contents).map_err(|error| manifest_error(&source, error.to_string()))?;

    if document.schema_version != 1 {
        return Err(manifest_error(
            &source,
            format!(
                "unsupported schema_version {}; this runner supports version 1",
                document.schema_version
            ),
        ));
    }
    if document.cases.is_empty() {
        return Err(manifest_error(&source, "at least one case is required"));
    }
    document.defaults.validate(&source, "defaults")?;
    let output_dir = if let Some(output_dir) = document.output_dir {
        validate_nonempty(&source, "output_dir", &output_dir)?;
        root.join(output_dir)
    } else {
        root.join("target/gungraun")
    };

    let mut seen = HashSet::new();
    let mut case_ids = Vec::with_capacity(document.cases.len());
    let mut benches = Vec::with_capacity(document.cases.len());
    let mut uncapped = Vec::new();
    for case in document.cases {
        let (id, settings) = case.into_settings();
        validate_id(&source, &id)?;
        if !seen.insert(id.to_ascii_lowercase()) {
            return Err(manifest_error(
                &source,
                format!("duplicate case id '{id}': IDs must also differ ignoring ASCII case"),
            ));
        }
        let scope = format!("case '{id}'");
        settings.validate(&source, &scope)?;
        let settings = settings.inherit(&document.defaults);
        let ir = settings.callgrind.hard_limits.ir;
        if ir.is_none() {
            if strict {
                return Err(manifest_error(
                    &source,
                    format!("{scope} requires a positive callgrind.hard_limits.Ir in check mode"),
                ));
            }
            uncapped.push(id.clone());
        }

        let program = settings
            .program
            .as_deref()
            .ok_or_else(|| manifest_error(&source, format!("{scope} is missing program")))?;
        let program = resolve_program(&source, &root, &scope, program, &settings.env)?;
        let mut callgrind = ToolSpec::new(Tool::Callgrind);
        callgrind.entry_point = Some(
            settings
                .callgrind
                .entry_point
                .map_or(EntryPoint::None, EntryPoint::Custom),
        );
        // An explicit empty regression configuration also avoids inventing relative limits for
        // uncapped captures. Reviewed absolute limits always use the existing runner machinery.
        callgrind.regression_config =
            Some(ToolRegressionConfig::Callgrind(CallgrindRegressionConfig {
                fail_fast: Some(false),
                hard_limits: ir
                    .map(|limit| (EventKind::Ir.into(), Limit::Int(limit)))
                    .into_iter()
                    .collect(),
                soft_limits: Vec::new(),
            }));
        let command = Command {
            args: settings
                .args
                .unwrap_or_default()
                .into_iter()
                .map(OsString::from)
                .collect(),
            config: BinaryBenchmarkConfig {
                current_dir: Some(root.join(settings.cwd.as_deref().unwrap_or("."))),
                env_clear: Some(true),
                envs: settings
                    .env
                    .into_iter()
                    .map(|(key, value)| (OsString::from(key), Some(OsString::from(value))))
                    .collect(),
                exit_with: Some(ExitWith::Code(settings.expected_exit.unwrap_or(0))),
                sandbox: Some(Sandbox {
                    enabled: Some(false),
                    ..Sandbox::default()
                }),
                tool_specs: ToolSpecs(vec![callgrind]),
                ..BinaryBenchmarkConfig::default()
            },
            path: program,
            ..Command::default()
        };
        benches.push(BinaryBenchmarkBench {
            args: None,
            command: CommandKind::Default(Box::new(command)),
            config: None,
            consts_display: None,
            function_name: id.clone(),
            has_setup: false,
            has_teardown: false,
            id: None,
        });
        case_ids.push(id);
    }

    Ok(Manifest {
        case_ids,
        groups: BinaryBenchmarkGroups {
            command_line_args: Vec::new(),
            config: BinaryBenchmarkConfig::default(),
            default_tool: Tool::Callgrind,
            groups: vec![BinaryBenchmarkGroup {
                binary_benchmarks: vec![BinaryBenchmark {
                    benches,
                    config: None,
                }],
                id: GROUP_ID.to_owned(),
                max_parallel: Some(1),
                ..BinaryBenchmarkGroup::default()
            }],
            has_setup: false,
            has_teardown: false,
        },
        output_dir,
        root,
        source,
        uncapped,
    })
}

fn manifest_error<T: Into<String>>(source: &Path, message: T) -> Error {
    Error::ManifestError(source.to_owned(), message.into())
}

fn resolve_program(
    source: &Path,
    root: &Path,
    scope: &str,
    program: &str,
    env: &BTreeMap<String, String>,
) -> Result<PathBuf, Error> {
    if program.contains(std::path::MAIN_SEPARATOR) {
        // Do not check existence here: launch failures belong to the normal execution flow, which
        // retains the case's native output directory and error evidence.
        Ok(root.join(program))
    } else {
        let search_path = env
            .get("PATH")
            .map(OsString::from)
            .or_else(|| std::env::var_os("PATH"));
        which::which_in(program, search_path, root).map_err(|error| {
            manifest_error(
                source,
                format!("{scope}: cannot find program '{program}' in PATH: {error}"),
            )
        })
    }
}

fn validate_id(source: &Path, id: &str) -> Result<(), Error> {
    // ToolOutputPath sanitizes names and truncates at 200 bytes. This restricted alphabet and
    // smaller bound prevent distinct manifest IDs from colliding in that existing output layout.
    if id.len() > 100
        || !id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(manifest_error(
            source,
            format!(
                "invalid case id '{id}': use 1 to 100 ASCII letters, digits, '.', '_' or '-', \
                 starting with a letter or digit"
            ),
        ));
    }
    Ok(())
}

fn validate_nonempty(source: &Path, field: &str, value: &str) -> Result<(), Error> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(manifest_error(
            source,
            format!("{field} must be nonempty and must not contain NUL bytes"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn bare_program_uses_declared_path_and_explicit_paths_do_not_require_existence() {
        let executable = std::env::current_exe().unwrap();
        let (_dir, path) = write_manifest(&format!(
            "schema_version = 1\n[[cases]]\nid = 'from_path'\nprogram = '{}'\n[cases.env]\nPATH = \
             '{}'\n[[cases]]\nid = 'absolute'\nprogram = '/missing/program'\n",
            executable.file_name().unwrap().to_string_lossy(),
            executable.parent().unwrap().display()
        ));
        let manifest = load(&path, false).unwrap();
        assert_eq!(command(&manifest, 0).path, executable);
        assert_eq!(command(&manifest, 1).path, Path::new("/missing/program"));
    }

    #[test]
    fn capture_reports_uncapped_cases_and_check_rejects_them() {
        let (_dir, path) = write_manifest(
            "schema_version = 1\n[[cases]]\nid = 'case'\nprogram = './not-built-yet'\n",
        );
        let manifest = load(&path, false).unwrap();
        assert_eq!(manifest.uncapped, ["case"]);
        assert_eq!(manifest.output_dir, manifest.root.join("target/gungraun"));
        let config = &command(&manifest, 0).config;
        assert_eq!(config.tool_specs.0[0].entry_point, Some(EntryPoint::None));
        assert_eq!(config.sandbox.as_ref().unwrap().enabled, Some(false));
        assert_eq!(config.exit_with, Some(ExitWith::Code(0)));
        assert!(
            load(&path, true)
                .unwrap_err()
                .to_string()
                .contains("case 'case' requires a positive")
        );
    }

    fn command(manifest: &Manifest, index: usize) -> &Command {
        let CommandKind::Default(command) =
            &manifest.groups.groups[0].binary_benchmarks[0].benches[index].command
        else {
            panic!("a manifest case must map to one command");
        };
        command
    }

    #[test]
    fn defaults_merge_fields_without_losing_boundary_or_environment() {
        let (_dir, path) = write_manifest(
            r#"
schema_version = 1
output_dir = "results with spaces"
[defaults]
program = "./bin/work load"
cwd = "fixtures"
args = ["inherited"]
expected_exit = 7
[defaults.env]
LC_ALL = "C"
KEY = "default"
[defaults.callgrind]
entry_point = "named_region"
[defaults.callgrind.hard_limits]
Ir = 20

[[cases]]
id = "Quote.Single.Owner"
args = ["", "$HOME", "$(touch marker)", "two words", "*.txt"]
[cases.env]
KEY = "override"
LITERAL = "${HOME}"
[cases.callgrind.hard_limits]
Ir = 10

[[cases]]
id = "Quote.Single.Kernel"
cwd = "."
expected_exit = 0
[cases.callgrind]
entry_point = "other_region"
"#,
        );
        let manifest = load(&path, true).unwrap();
        let root = path.parent().unwrap().canonicalize().unwrap();
        assert_eq!(manifest.root, root);
        assert_eq!(manifest.source, path.canonicalize().unwrap());
        assert_eq!(manifest.output_dir, root.join("results with spaces"));
        assert_eq!(manifest.uncapped, Vec::<String>::new());
        assert_eq!(
            manifest.case_ids,
            ["Quote.Single.Owner", "Quote.Single.Kernel"]
        );
        let first = command(&manifest, 0);
        assert_eq!(first.path, root.join("bin/work load"));
        assert_eq!(first.config.current_dir, Some(root.join("fixtures")));
        assert_eq!(first.config.env_clear, Some(true));
        assert_eq!(first.config.exit_with, Some(ExitWith::Code(7)));
        assert_eq!(
            first.args,
            ["", "$HOME", "$(touch marker)", "two words", "*.txt"].map(OsString::from)
        );
        let env = first.config.resolve_envs();
        assert_eq!(env[&OsString::from("KEY")], "override");
        assert_eq!(env[&OsString::from("LC_ALL")], "C");
        assert_eq!(env[&OsString::from("LITERAL")], "${HOME}");
        let callgrind = &first.config.tool_specs.0[0];
        assert_eq!(
            callgrind.entry_point,
            Some(EntryPoint::Custom("named_region".to_owned()))
        );
        assert_eq!(
            callgrind.regression_config,
            Some(ToolRegressionConfig::Callgrind(CallgrindRegressionConfig {
                fail_fast: Some(false),
                hard_limits: vec![(EventKind::Ir.into(), Limit::Int(10))],
                soft_limits: Vec::new(),
            }))
        );
        let second = command(&manifest, 1);
        assert_eq!(second.args, [OsString::from("inherited")]);
        assert_eq!(second.config.current_dir, Some(root));
        assert_eq!(second.config.exit_with, Some(ExitWith::Code(0)));
        assert_eq!(
            second.config.tool_specs.0[0].entry_point,
            Some(EntryPoint::Custom("other_region".to_owned()))
        );
        let Some(ToolRegressionConfig::Callgrind(regression)) =
            &second.config.tool_specs.0[0].regression_config
        else {
            panic!("expected Callgrind regression configuration");
        };
        assert_eq!(
            regression.hard_limits,
            [(EventKind::Ir.into(), Limit::Int(20))]
        );
    }

    #[test]
    fn duplicate_ids_and_names_that_would_be_truncated_are_rejected() {
        let (_dir, path) = write_manifest(
            "schema_version = 1\n[defaults]\nprogram = './binary'\n[[cases]]\nid = \
             'duplicate'\n[[cases]]\nid = 'duplicate'\n",
        );
        assert!(
            load(&path, false)
                .unwrap_err()
                .to_string()
                .contains("duplicate case id 'duplicate'")
        );
        fs::write(
            &path,
            "schema_version = 1\n[defaults]\nprogram = './binary'\n[[cases]]\nid = \
             'Quote.Owner'\n[[cases]]\nid = 'quote.owner'\n",
        )
        .unwrap();
        assert!(
            load(&path, false)
                .unwrap_err()
                .to_string()
                .contains("ignoring ASCII case")
        );
        let contents = format!(
            "schema_version = 1\n[[cases]]\nid = '{}'\nprogram = './binary'\n",
            "x".repeat(101)
        );
        fs::write(&path, contents).unwrap();
        assert!(
            load(&path, false)
                .unwrap_err()
                .to_string()
                .contains("1 to 100 ASCII")
        );
    }

    #[rstest]
    #[case::schema("schema_version = 2\ncases = []", "unsupported schema_version 2")]
    #[case::empty("schema_version = 1\ncases = []", "at least one case")]
    #[case::version_missing("cases = []", "schema_version")]
    #[case::unknown_top(
        "schema_version = 1\ncases = []\nfilter = 'case'",
        "unknown field `filter`"
    )]
    #[case::unknown_defaults(
        "schema_version = 1\ncases = []\n[defaults]\nprogramm = 'x'",
        "unknown field `programm`"
    )]
    #[case::unknown_case(
        "schema_version = 1\n[[cases]]\nid = 'case'\narguments = []",
        "unknown field `arguments`"
    )]
    #[case::unknown_callgrind(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind]\nentrypoint = 'f'",
        "unknown field `entrypoint`"
    )]
    #[case::unknown_event(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind.hard_limits]\nIr_typo = 10",
        "unknown field `Ir_typo`"
    )]
    #[case::unsupported_event(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind.hard_limits]\nDr = 10",
        "unknown field `Dr`"
    )]
    #[case::float_limit(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind.hard_limits]\nIr = 1.5",
        "invalid type"
    )]
    #[case::negative_limit(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind.hard_limits]\nIr = -1",
        "invalid value"
    )]
    #[case::zero_limit(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind.hard_limits]\nIr = 0",
        "must be greater than zero"
    )]
    #[case::missing_program("schema_version = 1\n[[cases]]\nid = 'case'", "missing program")]
    #[case::empty_program(
        "schema_version = 1\n[[cases]]\nid = 'case'\nprogram = ''",
        "program must be nonempty"
    )]
    #[case::empty_entry(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.callgrind]\nentry_point = ''",
        "entry_point must be nonempty"
    )]
    #[case::exit_range(
        "schema_version = 1\n[[cases]]\nid = 'case'\nexpected_exit = 256",
        "between 0 and 255"
    )]
    #[case::negative_exit(
        "schema_version = 1\n[[cases]]\nid = 'case'\nexpected_exit = -1",
        "between 0 and 255"
    )]
    #[case::env_key(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.env]\n'a=b' = 'literal'",
        "invalid environment entry"
    )]
    #[case::empty_env_key(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.env]\n'' = 'literal'",
        "invalid environment entry"
    )]
    #[case::nul_arg(
        "schema_version = 1\n[[cases]]\nid = 'case'\nargs = [\"\\u0000\"]",
        "args must not contain NUL"
    )]
    #[case::nul_env(
        "schema_version = 1\n[[cases]]\nid = 'case'\n[cases.env]\nKEY = \"\\u0000\"",
        "invalid environment entry"
    )]
    #[case::empty_output(
        "schema_version = 1\noutput_dir = ''\n[[cases]]\nid = 'case'",
        "output_dir must be nonempty"
    )]
    fn invalid_configuration_is_rejected(#[case] contents: &str, #[case] diagnostic: &str) {
        let (_dir, path) = write_manifest(contents);
        let error = load(&path, false).unwrap_err().to_string();
        assert!(error.contains(diagnostic), "{error}");
        assert!(error.contains("perf guard.toml"), "{error}");
    }

    #[rstest]
    #[case("")]
    #[case("../escape")]
    #[case("a/b")]
    #[case("a:b")]
    #[case("a b")]
    #[case("é")]
    #[case("_leading")]
    fn unsafe_or_ambiguous_ids_are_rejected(#[case] id: &str) {
        let (_dir, path) = write_manifest(&format!(
            "schema_version = 1\n[[cases]]\nid = '{id}'\nprogram = './binary'\n"
        ));
        assert!(
            load(&path, false)
                .unwrap_err()
                .to_string()
                .contains("invalid case id")
        );
    }

    fn write_manifest(contents: &str) -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("consumer with spaces");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("perf guard.toml");
        fs::write(&path, contents).unwrap();
        (dir, path)
    }
}
