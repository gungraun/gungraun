use std::path::PathBuf;

use anyhow::{Result, anyhow};
use gungraun::Tool;
use gungraun_runner::runner::callgrind::hashmap_parser::{CallgrindMap, HashMapParser};
use gungraun_runner::runner::callgrind::parser::CallgrindParser;
use gungraun_runner::runner::common::ModulePath;
use gungraun_runner::runner::tool::path::{ToolOutputPath, ToolOutputPathKind};
use gungraun_runner::summary::model::{BaselineKind, BenchmarkSummary};

use crate::common::Summary;

#[expect(unused)]
#[derive(Debug)]
pub struct Assert {
    module_path: ModulePath,
    group: String,
    function: String,
    id: String,
    workspace_root: PathBuf,
    target_dir: PathBuf,
    out_dir: PathBuf,
    out_path: PathBuf,
    summary_path: Option<PathBuf>,
}

impl Assert {
    pub fn new(module_path: &str, group: &str, function: &str, id: &str) -> Result<Self> {
        let name = format!("{function}.{id}");
        let package = env!("CARGO_PKG_NAME");

        let meta = cargo_metadata::MetadataCommand::new().exec()?;
        let target_dir = meta
            .workspace_root
            .join("target/gungraun")
            .join(package)
            .into_std_path_buf();

        let out_dir = target_dir.join(module_path).join(group).join(&name);

        let out_path = out_dir.join(format!("callgrind.{name}.out"));

        if !out_path.exists() {
            panic!(
                "Callgrind output file '{}' should exist",
                out_path.display()
            );
        }

        let summary_path = out_dir.join("summary.json");

        Ok(Self {
            module_path: ModulePath::new(module_path),
            group: group.to_owned(),
            function: function.to_owned(),
            id: id.to_owned(),
            workspace_root: meta.workspace_root.into_std_path_buf(),
            target_dir,
            out_dir,
            out_path,
            summary_path: summary_path.exists().then_some(summary_path),
        })
    }

    /// Asserts that `assert` returns true and panics if it returns false
    ///
    /// `assert` takes the deserialized json from the `summary.json` as input and returns a boolean.
    /// The input is the [`gungraun_runner::summary::model::BenchmarkSummary`] struct.
    ///
    /// # Errors
    ///
    /// If the summary.json file did not exist
    #[track_caller]
    pub fn summary(&self, assert: fn(BenchmarkSummary) -> bool) -> Result<()> {
        let summary = Summary::new(
            self.summary_path
                .as_ref()
                .ok_or_else(|| anyhow!("The summary.json should exist"))?,
        )?;

        assert!(assert(summary.0));

        Ok(())
    }

    /// Asserts that `assert` returns true and panics if it returns false
    ///
    /// The `assert` closure can make assertions based on the
    /// [`gungraun_runner::runner::callgrind::hashmap_parser::CallgrindMap`]. The assert
    /// function is supposed to return a boolean.
    ///
    /// In the presence of multiple output files, threads, subprocesses only the total can be
    /// asserted.
    ///
    /// # Errors
    ///
    /// If the summary.json file did not exist
    #[track_caller]
    pub fn callgrind_map(&self, assert: fn(CallgrindMap) -> bool) -> Result<()> {
        let parser = HashMapParser {
            project_root: self.workspace_root.clone(),
            sentinel: None,
        };

        let maps = parser
            .parse(
                &ToolOutputPath::new(
                    ToolOutputPathKind::Out,
                    Tool::Callgrind,
                    &BaselineKind::Old,
                    &self.target_dir,
                    &self.module_path.join(&self.group),
                    &format!("{}.{}", self.function, self.id),
                    false,
                )
                .unwrap(),
            )
            .unwrap();

        let mut total = CallgrindMap::default();
        for (_, _, map) in &maps {
            total.add_mut(map);
        }

        assert!(assert(total));

        Ok(())
    }
}
