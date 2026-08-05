//! The module containing the [`ToolOutputPath`] and other related elements

use std::collections::HashMap;
use std::fmt::{Display, Write as FmtWrite};
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use anyhow::{Context, Result};
use derive_more::Deref;
use log::{debug, log_enabled};
use regex::{Captures, Regex};
use tempfile::{Builder, TempDir};

use crate::api::Tool;
use crate::runner::callgrind;
use crate::runner::common::ModulePath;
use crate::summary::model::{BaselineKind, BaselineName};
use crate::util::truncate_str_utf8;

/// Sanitized output paths grouped by optional perf part number.
///
/// Each entry contains the paths for one optional `p<N>` part and the remaining modifier string,
/// such as `cal` or `overhead`, when one is present.
pub type OutputPathParts = HashMap<Option<u64>, Vec<(PathBuf, Option<String>)>>;

// This regex matches the original file name without the prefix as it is created by callgrind.
// The baseline <name> (base@<name>) can only consist of ascii and underscore characters.
// Flamegraph files are ignored by this regex
//
// Note callgrind doesn't support xtree, xleak files
static CALLGRIND_ORIG_FILENAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "^(?<type>[.](out|log))(?<base>[.](old|base@[^.-]+))?",
        "(?<pid>[.][#][0-9]+)?(?<part>[.][0-9]+)?(?<thread>-[0-9]+)?$"
    ))
    .expect("Regex should compile")
});

/// This regex matches the original file name without the prefix as it is created by bbv
///
/// Note bbv doesn't support xtree, xleak files
static BBV_ORIG_FILENAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "^(?<type>[.](?:out|log))(?<base>[.](old|base@[^.]+))?",
        "(?<bbv_type>[.](?:bb|pc))?(?<pid>[.][#][0-9]+)?(?<thread>[.][0-9]+)?$"
    ))
    .expect("Regex should compile")
});

/// This regex matches the original file name without the prefix as it is created by all tools
/// other than callgrind, bbv and perf.
static GENERIC_ORIG_FILENAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "^(?<type>[.](?:out|log|xtree|xleak|data))(?<base>[.](old|base@[^.]+))?",
        "(?<pid>[.][#][0-9]+)?(?<ext>[.][^#]+)?$",
    ))
    .expect("Regex should compile")
});

/// This regex matches the original file name without the prefix as it is created by perf
///
/// This regex doesn't match *.cal.XXX files which are created during calibration. These files
/// should be cleaned up during calibration.
static PERF_ORIG_FILENAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "^(?<type>[.](?:out|log|data))(?<base>[.](old|base@[^.]+))?",
        "(?<part>[.]p[0-9]+)?(?<ext>[.][^0-9]+)?$",
    ))
    .expect("Regex should compile")
});

#[derive(Debug)]
enum SanitizableBaseline {
    Baseline(String),
    NoBaseline,
    OldBaseline,
}

/// The different output path kinds
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutputPathKind {
    /// The output path for `*.out` files
    Out,
    /// The output path for `*.out.old` files
    OldOut,
    /// The output path for baseline `out` files
    BaseOut(String),
    /// The output path for `*.log` files
    Log,
    /// The output path for `*.log.old` files
    OldLog,
    /// The output path for baseline `log` files
    BaseLog(String),
    /// The output path for `*.data` files
    Data,
    /// The output path for `*.data.old` files
    OldData,
    /// The output path for baseline `data` files
    BaseData(String),
    /// The output path for `*.xtree` files
    Xtree,
    /// The output path for `*.xtree.old` files
    OldXtree,
    /// The output for baseline `xtree` files
    BaseXtree(String),
    /// The output path for `*.xleak` files
    Xleak,
    /// The output path for `*.xleak.old` files
    OldXleak,
    /// The output for baseline `xleak` files
    BaseXleak(String),
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct BbvTypeKey {
    baseline: Option<String>,
    bbv_type: Option<String>,
    output_type: String,
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct CallgrindTypeKey {
    baseline: Option<String>,
    output_type: String,
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct GenericTypeKey {
    baseline: Option<String>,
    output_type: String,
}

#[derive(Debug)]
struct OriginalBbvFile {
    path: PathBuf,
    thread: usize,
}

#[derive(Debug)]
struct OriginalCallgrindFile {
    part: Option<u64>,
    path: PathBuf,
}

#[derive(Debug)]
struct OriginalGenericFile {
    extension: Option<String>,
    path: PathBuf,
    pid: Option<u32>,
}

#[derive(Debug)]
struct OriginalPerfFile {
    part: Option<u64>,
    path: PathBuf,
}

#[derive(Debug, Deref)]
struct PathSanitizer<'a> {
    output_path: &'a ToolOutputPath,
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct PerfTypeKey {
    baseline: Option<String>,
    output_type: String,
}

#[derive(Debug)]
struct SanitizedFileNameBuilder(String);

/// The tool specific output path(s)
///
/// In the presence of a temporary directory, the temporary directory is assumed to be the output
/// path for any new files from the Valgrind tools and files in it are returned by methods like
/// [`ToolOutputPath::sanitized_paths`] after [`ToolOutputPath::sanitize`]. Otherwise the benchmark
/// directory contains the new and "old" files of previous benchmark runs. The temporary files need
/// to be transferred to the benchmark directory manually for example with
/// [`ToolOutputPath::copy_temp`] and doesn't happen on drop.
///
/// If a temporary directory for the new files exists, it's best to use it as long as possible for
/// example for parsing since nowadays the temporary directory is most likely stored an in-memory
/// filesystem (i.e. tmpfs) which avoids the more expensive real disk IO.
#[derive(Debug, Clone)]
pub struct ToolOutputPath {
    /// The [`BaselineKind`]
    pub baseline_kind: BaselineKind,
    /// The final directory of all the output files
    pub dir: PathBuf,
    /// The [`ToolOutputPathKind`]
    pub kind: ToolOutputPathKind,
    /// The modifiers which are prepended to the extension
    pub modifiers: Vec<String>,
    /// The name of this output path
    pub name: String,
    /// The temporary directory for the new valgrind output
    pub temp: Option<Arc<TempDir>>,
    /// The tool
    pub tool: Tool,
}

impl ToolOutputPath {
    /// Create a new `ToolOutputPath`.
    ///
    /// The `base_dir` is supposed to be the same as [`crate::runner::meta::Metadata::target_dir`].
    /// The `name` is supposed to be the name of the benchmark function. If a benchmark id is
    /// present join both with a dot as separator to get the final `name`.
    pub fn new(
        kind: ToolOutputPathKind,
        tool: Tool,
        baseline_kind: &BaselineKind,
        base_dir: &Path,
        module: &ModulePath,
        name: &str,
        use_temp_dir: bool,
    ) -> Result<Self> {
        let current = base_dir;
        let module_path: PathBuf = module.to_string().split("::").collect();
        let sanitized_name = sanitize_filename::sanitize_with_options(
            name,
            sanitize_filename::Options {
                windows: false,
                truncate: false,
                replacement: "_",
            },
        );
        let sanitized_name = truncate_str_utf8(&sanitized_name, 200);
        let temp = use_temp_dir
            .then(|| {
                Builder::new()
                    .prefix("gungraun.tmp")
                    .suffix(sanitized_name)
                    .rand_bytes(10)
                    .tempdir()
                    .map(Arc::new)
            })
            .transpose()?;

        Ok(Self {
            kind,
            tool,
            baseline_kind: baseline_kind.clone(),
            dir: current
                .join(base_dir)
                .join(module_path)
                .join(sanitized_name),
            name: sanitized_name.to_owned(),
            modifiers: vec![],
            temp,
        })
    }

    /// Initialize and create the output directory and organize files
    ///
    /// This method moves the old output to `$TOOL_ID.*.out.old`
    pub fn with_init(
        kind: ToolOutputPathKind,
        tool: Tool,
        baseline_kind: &BaselineKind,
        base_dir: &Path,
        module: &str,
        name: &str,
        use_temp_dir: bool,
    ) -> Result<Self> {
        Self::new(
            kind,
            tool,
            baseline_kind,
            base_dir,
            &ModulePath::new(module),
            name,
            use_temp_dir,
        )
        .and_then(|o| o.init().map(|()| o))
    }

    /// Initialize the directory in which the final files are stored
    pub fn init(&self) -> Result<()> {
        debug!("Initializing benchmark directory: '{}'", self.dir.display());
        std::fs::create_dir_all(&self.dir).with_context(|| {
            format!(
                "Failed to create benchmark directory: '{}'",
                self.dir.display()
            )
        })
    }

    /// Remove the sanitized files of this output path
    pub fn clear(&self) -> Result<()> {
        for entry in self.sanitized_paths_in(&self.dir)? {
            debug!("Clearing '{}'", entry.display());
            std::fs::remove_file(&entry).with_context(|| {
                format!("Failed to remove benchmark file: '{}'", entry.display())
            })?;
        }

        self.clear_temp_files(false)
    }

    /// Remove sanitized files for the given part if their modifier matches one of `modifiers`.
    ///
    /// Files without modifiers are kept. Passing `None` for `part` targets files without a `p<N>`
    /// part suffix.
    ///
    /// # Errors
    ///
    /// Returns an error if output paths cannot be read or if removing a matching file fails.
    pub fn clear_part_with_modifiers(&self, part: Option<u64>, modifiers: &[&str]) -> Result<()> {
        let parts = self.sanitized_paths_by_part()?;

        if let Some(paths) = parts.get(&part) {
            for (path, real_modifiers) in paths {
                if let Some(real_modifiers) = real_modifiers
                    && modifiers.contains(&real_modifiers.as_str())
                {
                    debug!(
                        "Clearing part {part:?} with modifier {real_modifiers}: {}",
                        path.display()
                    );
                    std::fs::remove_file(path)?;
                }
            }
        }

        Ok(())
    }

    /// Delete temporary/unsanitized files in the benchmark directory
    ///
    /// This method does not operate in the temporary directory of this output path if it is present
    /// and always uses the benchmark directory for the cleanup.
    ///
    /// As long as files are not sanitized they are suffixed with `.#PID` where PID can be `0` (this
    /// is intentionally set by us to show its artificial nature and to not collide with real pids)
    /// or any other number and optional other suffixes added by valgrind tools.
    ///
    /// # Errors
    ///
    /// If the benchmark directory does not exist or if there are IO errors during the deletion and
    /// when reading the directory content.
    pub fn clear_temp_files(&self, ignore_tool: bool) -> Result<()> {
        let pattern = if ignore_tool {
            format!("{}/*.{}.*.#*", self.dir.display(), self.name)
        } else {
            format!("{}/{}.{}.*.#*", self.dir.display(), self.tool, self.name)
        };
        for entry in glob::glob(&pattern).expect("The glob pattern should be valid") {
            let entry = entry?;
            debug!("Clearing temporary file '{}'", entry.display());
            std::fs::remove_file(&entry).with_context(|| {
                format!("Failed to remove temporary file: '{}'", entry.display())
            })?;
        }

        Ok(())
    }

    /// Remove the sanitized old or base files and rename the present files to "old" files
    pub fn shift(&self) -> Result<()> {
        match self.baseline_kind {
            BaselineKind::Old => {
                self.to_base_path().clear()?;
                for entry in self.sanitized_paths_in(&self.dir)? {
                    let extension = entry.extension().expect("An extension should be present");
                    let mut extension = extension.to_owned();
                    extension.push(".old");
                    let new_path = entry.with_extension(extension);

                    debug!(
                        "Moving file from '{}' to '{}'",
                        entry.display(),
                        new_path.display()
                    );
                    std::fs::rename(&entry, &new_path).with_context(|| {
                        format!(
                            "Failed to move benchmark file from '{}' to '{}'",
                            entry.display(),
                            new_path.display()
                        )
                    })?;
                }
                Ok(())
            }
            BaselineKind::Name(_) => self.clear(),
        }
    }

    /// Copies all files in the temporary directory to the benchmark directory
    ///
    /// This method operates on all files independent of the tool, output path kind, sanitization,
    /// ... If there is no temporary directory then this method does nothing.
    ///
    /// # Errors
    ///
    /// If reading the content of the temporary directory fails or there are IO errors during the
    /// copy call.
    pub fn copy_temp(&self) -> Result<()> {
        if let Some(temp) = &self.temp {
            for entry in std::fs::read_dir(temp.path())? {
                let entry = entry?;
                let file_name = entry.file_name();
                let dest_path = self.dir.join(&file_name);
                let src_path = entry.path();

                debug!(
                    "Copying '{}' from temporary directory to '{}' in the benchmark directory",
                    src_path.display(),
                    dest_path.display()
                );
                std::fs::copy(src_path, dest_path)?;
            }
        }

        Ok(())
    }

    /// Moves all files of the temporary directory to the benchmark directory
    ///
    /// Like [`Self::copy_temp`] this method does not care about the tool, output path kind,
    /// sanitization, ... and does nothing if there is no temporary directory. This method does not
    /// delete the temporary directory itself which is still usable if required.
    ///
    /// # Errors
    ///
    /// If reading the content of the temporary directory fails or there are IO errors during the
    /// copy call.
    pub fn move_temp(&self) -> Result<()> {
        if let Some(temp) = &self.temp {
            for entry in std::fs::read_dir(temp.path())? {
                let entry = entry?;
                let file_name = entry.file_name();
                let dest_path = self.dir.join(&file_name);
                let src_path = entry.path();

                debug!(
                    "Moving '{}' from temporary directory to '{}' in the benchmark directory",
                    src_path.display(),
                    dest_path.display()
                );
                std::fs::copy(&src_path, dest_path)?;
                std::fs::remove_file(src_path)?;
            }
        }

        Ok(())
    }

    /// Returns the destination directory for new files.
    ///
    /// In the presence of a temporary directory this is the temporary directory, otherwise it is
    /// the benchmark directory. Neither directory needs to exist for this method.
    pub fn dest_dir(&self) -> &Path {
        if let Some(temp) = self.temp.as_ref() {
            temp.path()
        } else {
            &self.dir
        }
    }

    /// Returns the name of the baseline if present.
    pub fn baseline_name(&self) -> Option<&BaselineName> {
        match &self.baseline_kind {
            BaselineKind::Old => None,
            BaselineKind::Name(baseline_name) => Some(baseline_name),
        }
    }

    /// Returns the name of the loaded baseline (as set by --load-baseline) if present.
    pub fn loaded_baseline_name(&self) -> Option<BaselineName> {
        match &self.kind {
            ToolOutputPathKind::BaseOut(name)
            | ToolOutputPathKind::BaseLog(name)
            | ToolOutputPathKind::BaseData(name)
            | ToolOutputPathKind::BaseXtree(name)
            | ToolOutputPathKind::BaseXleak(name) => Some(BaselineName(name.clone())),
            _ => None,
        }
    }

    /// Returns `true` if a sanitized file of this output path exists.
    pub fn exists(&self) -> bool {
        self.sanitized_paths().is_ok_and(|p| !p.is_empty())
    }

    /// Returns `true` if there are multiple sanitized files of this output path.
    pub fn is_multiple(&self) -> bool {
        self.sanitized_paths().is_ok_and(|p| p.len() > 1)
    }

    /// Return `true` if this output path is an old or baseline path
    pub fn is_base_path(&self) -> bool {
        match self.kind {
            ToolOutputPathKind::Out
            | ToolOutputPathKind::Log
            | ToolOutputPathKind::Data
            | ToolOutputPathKind::Xtree
            | ToolOutputPathKind::Xleak => false,
            ToolOutputPathKind::OldOut
            | ToolOutputPathKind::BaseOut(_)
            | ToolOutputPathKind::OldLog
            | ToolOutputPathKind::BaseLog(_)
            | ToolOutputPathKind::OldData
            | ToolOutputPathKind::BaseData(_)
            | ToolOutputPathKind::OldXtree
            | ToolOutputPathKind::BaseXtree(_)
            | ToolOutputPathKind::OldXleak
            | ToolOutputPathKind::BaseXleak(_) => true,
        }
    }

    /// Convert this output path to a base output path
    #[must_use]
    pub fn to_base_path(&self) -> Self {
        Self {
            kind: match (&self.kind, &self.baseline_kind) {
                (ToolOutputPathKind::Out, BaselineKind::Old) => ToolOutputPathKind::OldOut,
                (
                    ToolOutputPathKind::Out | ToolOutputPathKind::BaseOut(_),
                    BaselineKind::Name(name),
                ) => ToolOutputPathKind::BaseOut(name.to_string()),
                (ToolOutputPathKind::Log, BaselineKind::Old) => ToolOutputPathKind::OldLog,
                (
                    ToolOutputPathKind::Log | ToolOutputPathKind::BaseLog(_),
                    BaselineKind::Name(name),
                ) => ToolOutputPathKind::BaseLog(name.to_string()),
                (ToolOutputPathKind::Data, BaselineKind::Old) => ToolOutputPathKind::OldData,
                (
                    ToolOutputPathKind::Data | ToolOutputPathKind::BaseData(_),
                    BaselineKind::Name(name),
                ) => ToolOutputPathKind::BaseData(name.to_string()),
                (ToolOutputPathKind::Xtree, BaselineKind::Old) => ToolOutputPathKind::OldXtree,
                (
                    ToolOutputPathKind::Xtree | ToolOutputPathKind::BaseXtree(_),
                    BaselineKind::Name(name),
                ) => ToolOutputPathKind::BaseXtree(name.to_string()),
                (ToolOutputPathKind::Xleak, BaselineKind::Old) => ToolOutputPathKind::OldXleak,
                (
                    ToolOutputPathKind::Xleak | ToolOutputPathKind::BaseXleak(_),
                    BaselineKind::Name(name),
                ) => ToolOutputPathKind::BaseXleak(name.to_string()),
                (kind, _) => kind.clone(),
            },
            tool: self.tool,
            baseline_kind: self.baseline_kind.clone(),
            name: self.name.clone(),
            dir: self.dir.clone(),
            modifiers: self.modifiers.clone(),
            temp: self.temp.clone(),
        }
    }

    /// Convert this tool output to the corresponding perf data output.
    ///
    /// For [`Tool::Perf`], output and log kinds are mapped to [`ToolOutputPathKind::Data`] or
    /// [`ToolOutputPathKind::BaseData`] while preserving the directory, modifiers, name, temporary
    /// directory, and baseline kind. Non-perf output paths are returned unchanged.
    #[must_use]
    pub fn to_data_output(&self) -> Self {
        match self.tool {
            Tool::Perf => {
                let kind = match &self.kind {
                    ToolOutputPathKind::Out
                    | ToolOutputPathKind::OldOut
                    | ToolOutputPathKind::Log
                    | ToolOutputPathKind::OldLog
                    | ToolOutputPathKind::Xtree
                    | ToolOutputPathKind::OldXtree
                    | ToolOutputPathKind::Xleak
                    | ToolOutputPathKind::OldXleak => ToolOutputPathKind::Data,
                    ToolOutputPathKind::BaseOut(name)
                    | ToolOutputPathKind::BaseXleak(name)
                    | ToolOutputPathKind::BaseLog(name)
                    | ToolOutputPathKind::BaseXtree(name) => {
                        ToolOutputPathKind::BaseData(name.clone())
                    }
                    kind => kind.clone(),
                };
                Self {
                    baseline_kind: self.baseline_kind.clone(),
                    dir: self.dir.clone(),
                    kind,
                    modifiers: self.modifiers.clone(),
                    name: self.name.clone(),
                    temp: self.temp.clone(),
                    tool: self.tool,
                }
            }
            _ => self.clone(),
        }
    }

    /// Convert this tool output path to the output of another tool output path
    ///
    /// A tool with no `*.out` file is log-file based. If the other tool is a out-file based tool
    /// the [`ToolOutputPathKind`] will be converted and vice-versa. The "old" (base) type (a tool
    /// output converted with [`ToolOutputPath::to_base_path`]) will be converted to a new
    /// `ToolOutputPath`.
    #[must_use]
    pub fn to_tool_output(&self, tool: Tool) -> Self {
        let kind = if tool.has_output_file() {
            match &self.kind {
                ToolOutputPathKind::Log
                | ToolOutputPathKind::OldLog
                | ToolOutputPathKind::Xtree
                | ToolOutputPathKind::OldXtree
                | ToolOutputPathKind::Xleak
                | ToolOutputPathKind::OldXleak => ToolOutputPathKind::Out,
                ToolOutputPathKind::BaseLog(name)
                | ToolOutputPathKind::BaseXtree(name)
                | ToolOutputPathKind::BaseXleak(name) => ToolOutputPathKind::BaseOut(name.clone()),
                kind => kind.clone(),
            }
        } else {
            match &self.kind {
                ToolOutputPathKind::Out
                | ToolOutputPathKind::OldOut
                | ToolOutputPathKind::Xtree
                | ToolOutputPathKind::OldXtree
                | ToolOutputPathKind::Xleak
                | ToolOutputPathKind::OldXleak => ToolOutputPathKind::Log,
                ToolOutputPathKind::BaseOut(name)
                | ToolOutputPathKind::BaseXtree(name)
                | ToolOutputPathKind::BaseXleak(name) => ToolOutputPathKind::BaseLog(name.clone()),
                kind => kind.clone(),
            }
        };
        Self {
            tool,
            kind,
            baseline_kind: self.baseline_kind.clone(),
            name: self.name.clone(),
            dir: self.dir.clone(),
            modifiers: self.modifiers.clone(),
            temp: self.temp.clone(),
        }
    }

    /// Convert this tool output to the according log output
    ///
    /// All tools have a log output even the ones which are out-file based.
    #[must_use]
    pub fn to_log_output(&self) -> Self {
        Self {
            kind: match &self.kind {
                ToolOutputPathKind::Out
                | ToolOutputPathKind::OldOut
                | ToolOutputPathKind::Data
                | ToolOutputPathKind::OldData
                | ToolOutputPathKind::Xleak
                | ToolOutputPathKind::OldXleak
                | ToolOutputPathKind::Xtree
                | ToolOutputPathKind::OldXtree => ToolOutputPathKind::Log,
                ToolOutputPathKind::BaseOut(name)
                | ToolOutputPathKind::BaseData(name)
                | ToolOutputPathKind::BaseXtree(name)
                | ToolOutputPathKind::BaseXleak(name) => ToolOutputPathKind::BaseLog(name.clone()),
                kind => kind.clone(),
            },
            tool: self.tool,
            baseline_kind: self.baseline_kind.clone(),
            name: self.name.clone(),
            dir: self.dir.clone(),
            modifiers: self.modifiers.clone(),
            temp: self.temp.clone(),
        }
    }

    /// If possible, convert this tool output to the according xtree output
    ///
    /// Not all tools support xtree output files
    #[must_use]
    pub fn to_xtree_output(&self) -> Option<Self> {
        self.tool.has_xtree_file().then(|| Self {
            kind: match &self.kind {
                ToolOutputPathKind::Out
                | ToolOutputPathKind::OldOut
                | ToolOutputPathKind::Data
                | ToolOutputPathKind::OldData
                | ToolOutputPathKind::Xleak
                | ToolOutputPathKind::OldXleak
                | ToolOutputPathKind::Log
                | ToolOutputPathKind::OldLog => ToolOutputPathKind::Xtree,
                ToolOutputPathKind::BaseOut(name)
                | ToolOutputPathKind::BaseData(name)
                | ToolOutputPathKind::BaseLog(name)
                | ToolOutputPathKind::BaseXleak(name) => {
                    ToolOutputPathKind::BaseXtree(name.clone())
                }
                kind => kind.clone(),
            },
            tool: self.tool,
            baseline_kind: self.baseline_kind.clone(),
            name: self.name.clone(),
            dir: self.dir.clone(),
            modifiers: self.modifiers.clone(),
            temp: self.temp.clone(),
        })
    }

    /// If possible, convert this tool output to the according xleak output
    ///
    /// Not all tools support xleak output files
    #[must_use]
    pub fn to_xleak_output(&self) -> Option<Self> {
        self.tool.has_xleak_file().then(|| Self {
            kind: match &self.kind {
                ToolOutputPathKind::Out
                | ToolOutputPathKind::OldOut
                | ToolOutputPathKind::Data
                | ToolOutputPathKind::OldData
                | ToolOutputPathKind::Xtree
                | ToolOutputPathKind::OldXtree
                | ToolOutputPathKind::Log
                | ToolOutputPathKind::OldLog => ToolOutputPathKind::Xleak,
                ToolOutputPathKind::BaseOut(name)
                | ToolOutputPathKind::BaseLog(name)
                | ToolOutputPathKind::BaseXtree(name) => {
                    ToolOutputPathKind::BaseXleak(name.clone())
                }
                kind => kind.clone(),
            },
            tool: self.tool,
            baseline_kind: self.baseline_kind.clone(),
            name: self.name.clone(),
            dir: self.dir.clone(),
            modifiers: self.modifiers.clone(),
            temp: self.temp.clone(),
        })
    }

    /// Returns the path to the log file for the given `path`.
    ///
    /// `path` is supposed to be a path to a valid file in the directory of this [`ToolOutputPath`].
    pub fn log_path_of(&self, path: &Path) -> Option<PathBuf> {
        let (file_name, temp_path) = if let Some(temp) = &self.temp {
            let temp_path = temp.path();
            if let Ok(file_name) = path.strip_prefix(temp_path) {
                Ok((file_name, Some(temp_path)))
            } else {
                path.strip_prefix(&self.dir).map(|f| (f, None))
            }
        } else {
            path.strip_prefix(&self.dir).map(|f| (f, None))
        }
        .ok()?;

        if let Some(suffix) = self.strip_prefix(&file_name.to_string_lossy()) {
            let mut is_out = false;
            let mut string = self.prefix();
            for split in suffix.split('.').filter(|s| !s.is_empty()) {
                match split {
                    "out" | "xtree" | "xleak" | "data" => {
                        is_out = true;
                        string.push('.');
                        string.push_str("log");
                    }
                    "log" => return Some(path.to_owned()),
                    "bb" | "pc" => {}
                    // In perf each part and each modifier has an own log file
                    _ if self.tool == Tool::Perf => {
                        string.push('.');
                        string.push_str(split);
                    }
                    _ => {
                        let is_tid_or_part = (split.starts_with('t') || split.starts_with('p'))
                            && split[1..].bytes().all(|b| b.is_ascii_digit());

                        if !is_tid_or_part {
                            string.push('.');
                            string.push_str(split);
                        }
                    }
                }
            }

            if is_out {
                let dir = temp_path.unwrap_or(&self.dir);
                return Some(dir.join(string));
            }
        }

        None
    }

    /// If the [`log::Level`] matches, dump the content of the sanitized log file(s) into the
    /// `writer`
    pub fn dump_log<W>(&self, log_level: log::Level, writer: &mut W) -> Result<()>
    where
        W: Write,
    {
        if log_enabled!(log_level) {
            for path in self.sanitized_paths()? {
                log::log!(
                    log_level,
                    "{} log output '{}':",
                    self.tool.id(),
                    path.display()
                );

                let file = File::open(&path).with_context(|| {
                    format!(
                        "Error opening {} output file '{}'",
                        self.tool.id(),
                        path.display()
                    )
                })?;

                let mut reader = BufReader::new(file);
                std::io::copy(&mut reader, writer)?;
            }
        }
        Ok(())
    }

    /// This method can only be used to create the path passed to the tools
    ///
    /// The modifiers are extrapolated by the tools and won't match any real path name.
    pub fn extension(&self) -> String {
        match (&self.kind, self.modifiers.is_empty()) {
            (ToolOutputPathKind::Out, true) => "out".to_owned(),
            (ToolOutputPathKind::Out, false) => format!("out.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::Log, true) => "log".to_owned(),
            (ToolOutputPathKind::Log, false) => format!("log.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::Data, true) => "data".to_owned(),
            (ToolOutputPathKind::Data, false) => format!("data.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::OldOut, true) => "out.old".to_owned(),
            (ToolOutputPathKind::OldOut, false) => format!("out.old.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::OldLog, true) => "log.old".to_owned(),
            (ToolOutputPathKind::OldLog, false) => format!("log.old.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::OldData, true) => "data.old".to_owned(),
            (ToolOutputPathKind::OldData, false) => {
                format!("data.old.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::BaseOut(name), true) => format!("out.base@{name}"),
            (ToolOutputPathKind::BaseOut(name), false) => {
                format!("out.base@{name}.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::BaseLog(name), true) => {
                format!("log.base@{name}")
            }
            (ToolOutputPathKind::BaseLog(name), false) => {
                format!("log.base@{name}.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::BaseData(name), true) => format!("data.base@{name}"),
            (ToolOutputPathKind::BaseData(name), false) => {
                format!("data.base@{name}.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::Xtree, true) => "xtree".to_owned(),
            (ToolOutputPathKind::Xtree, false) => format!("xtree.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::OldXtree, true) => "xtree.old".to_owned(),
            (ToolOutputPathKind::OldXtree, false) => {
                format!("xtree.old.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::BaseXtree(name), true) => format!("xtree.base@{name}"),
            (ToolOutputPathKind::BaseXtree(name), false) => {
                format!("xtree.base@{name}.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::Xleak, true) => "xleak".to_owned(),
            (ToolOutputPathKind::Xleak, false) => format!("xleak.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::OldXleak, true) => "xleak.old".to_owned(),
            (ToolOutputPathKind::OldXleak, false) => {
                format!("xleak.old.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::BaseXleak(name), true) => format!("xleak.base@{name}"),
            (ToolOutputPathKind::BaseXleak(name), false) => {
                format!("xleak.base@{name}.{}", self.modifiers.join("."))
            }
        }
    }

    /// Creates new `ToolOutputPath` with `modifiers`.
    #[must_use]
    pub fn with_modifiers<I, T>(&self, modifiers: T) -> Self
    where
        I: Into<String>,
        T: IntoIterator<Item = I>,
    {
        Self {
            kind: self.kind.clone(),
            tool: self.tool,
            baseline_kind: self.baseline_kind.clone(),
            dir: self.dir.clone(),
            name: self.name.clone(),
            modifiers: modifiers.into_iter().map(Into::into).collect(),
            temp: self.temp.clone(),
        }
    }

    /// Creates a new `ToolOutputPath` with `modifiers` added to the existing modifiers
    #[must_use]
    pub fn with_added_modifiers<I, T>(&self, modifiers: T) -> Self
    where
        I: Into<String>,
        T: IntoIterator<Item = I>,
    {
        let mut this = self.clone();
        this.modifiers.extend(modifiers.into_iter().map(Into::into));
        this
    }

    /// Return the unexpanded path usable as input for `--callgrind-out-file`, ...
    ///
    /// The path returned by this method does not necessarily have to exist and can include
    /// modifiers like `%p`. Use [`Self::sanitized_paths`] to get the real and existing (possibly
    /// multiple) paths to the output files of the respective tool.
    pub fn to_path(&self) -> PathBuf {
        self.dest_dir().join(self.file_name())
    }

    /// Return the filename for this tool's output file
    ///
    /// The filename is constructed as `<tool_id>.<name>.<extension>`, where:
    /// - `tool_id` is the Valgrind tool identifier (e.g., "callgrind", "memcheck")
    /// - `name` is the benchmark name
    /// - `extension` is the file extension for this tool (e.g., "out", "log")
    ///
    /// For example, a Callgrind output file might be named `callgrind.bench_fibonacci.out`.
    pub fn file_name(&self) -> PathBuf {
        PathBuf::from(format!(
            "{}.{}.{}",
            self.tool.id(),
            self.name,
            self.extension()
        ))
    }

    /// Walk the benchmark directory (non-recursive)
    pub fn walk_dir(&self, dir: Option<&Path>) -> Result<impl Iterator<Item = DirEntry> + use<>> {
        let dir = if let Some(dir) = dir {
            dir
        } else if self.is_base_path() {
            &self.dir
        } else {
            self.dest_dir()
        };
        std::fs::read_dir(dir)
            .with_context(|| format!("Failed opening benchmark directory: '{}'", dir.display()))
            .map(|i| i.into_iter().filter_map(Result::ok))
    }

    /// Strip the `<tool>.<name>` prefix from a `file_name`
    pub fn strip_prefix<'a>(&self, file_name: &'a str) -> Option<&'a str> {
        file_name.strip_prefix(format!("{}.{}", self.tool.id(), self.name).as_str())
    }

    /// Returns the file name prefix as in `<tool>.<name>`.
    pub fn prefix(&self) -> String {
        format!("{}.{}", self.tool.id(), self.name)
    }

    /// Returns the [`sanitized`] paths of a tool's output files.
    ///
    /// A tool can have many output files so [`Self::to_path`] is not enough
    ///
    /// [`sanitized`]: Self::sanitize
    pub fn sanitized_paths(&self) -> Result<Vec<PathBuf>> {
        let dir = if self.is_base_path() {
            &self.dir
        } else {
            self.dest_dir()
        };
        self.sanitized_paths_in(dir)
    }

    /// Returns the [`sanitized`] paths of a tool's output files in this `dir`
    ///
    /// A tool can have many output files so [`Self::to_path`] is not enough
    ///
    /// [`sanitized`]: Self::sanitize
    #[expect(clippy::case_sensitive_file_extension_comparisons)]
    pub fn sanitized_paths_in(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = vec![];
        for entry in self.walk_dir(Some(dir))? {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            // Silently ignore all paths which don't follow this scheme, for example
            // (`summary.json`)
            if let Some(suffix) = self.strip_prefix(&file_name) {
                let is_match = || match &self.kind {
                    ToolOutputPathKind::Out => suffix.ends_with(".out"),
                    ToolOutputPathKind::Log => suffix.ends_with(".log"),
                    ToolOutputPathKind::Data => suffix.ends_with(".data"),
                    ToolOutputPathKind::OldOut => suffix.ends_with(".out.old"),
                    ToolOutputPathKind::OldLog => suffix.ends_with(".log.old"),
                    ToolOutputPathKind::OldData => suffix.ends_with(".data.old"),
                    ToolOutputPathKind::BaseOut(name) => {
                        suffix.ends_with(format!(".out.base@{name}").as_str())
                    }
                    ToolOutputPathKind::BaseLog(name) => {
                        suffix.ends_with(format!(".log.base@{name}").as_str())
                    }
                    ToolOutputPathKind::BaseData(name) => {
                        suffix.ends_with(format!(".data.base@{name}").as_str())
                    }
                    ToolOutputPathKind::Xtree => suffix.ends_with(".xtree"),
                    ToolOutputPathKind::OldXtree => suffix.ends_with(".xtree.old"),
                    ToolOutputPathKind::BaseXtree(name) => {
                        suffix.ends_with(format!(".xtree.base@{name}").as_str())
                    }
                    ToolOutputPathKind::Xleak => suffix.ends_with(".xleak"),
                    ToolOutputPathKind::OldXleak => suffix.ends_with(".xleak.old"),
                    ToolOutputPathKind::BaseXleak(name) => {
                        suffix.ends_with(format!(".xleak.base@{name}").as_str())
                    }
                };

                if is_match() {
                    paths.push(entry.path());
                }
            }
        }
        Ok(paths)
    }

    /// Returns the [`sanitized`] paths with their respective modifiers if present.
    ///
    /// [`sanitized`]: Self::sanitize
    pub fn sanitized_paths_with_modifier(&self) -> Result<Vec<(PathBuf, Option<String>)>> {
        let mut paths = vec![];
        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Silently ignore all paths which don't follow this pattern, for example
            // (`summary.json`)
            if let Some(suffix) = self.strip_prefix(&file_name) {
                let remainder = self.strip_output_kind(suffix);

                if let Some(remainder) = remainder {
                    paths.push((
                        entry.path(),
                        (!remainder.is_empty()).then(|| remainder.to_owned()),
                    ));
                }
            }
        }

        Ok(paths)
    }

    /// Returns sanitized paths grouped by optional part number.
    ///
    /// The grouping uses the modifier prefix left after removing the output kind. For example,
    /// `perf.bench.p1.overhead.out` is grouped under part `1` with modifier `overhead`, while
    /// `perf.bench.out` is grouped under `None` with no modifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the output directory cannot be read.
    pub fn sanitized_paths_by_part(&self) -> Result<OutputPathParts> {
        let mut paths = HashMap::new();
        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Silently ignore all paths which don't follow this pattern, for example
            // (`summary.json`)
            if let Some(suffix) = self.strip_prefix(&file_name) {
                let remainder = self.strip_output_kind(suffix);

                let Some(remainder) = remainder else {
                    continue;
                };

                let remainder = remainder.trim_start_matches('.');

                let (part, modifiers) = if remainder.is_empty() {
                    (None, None)
                } else if let Some((part, modifiers)) = remainder.split_once('.') {
                    if let Some(part) = part.strip_prefix('p').and_then(|p| p.parse::<u64>().ok()) {
                        (
                            Some(part),
                            (!modifiers.is_empty()).then(|| modifiers.to_owned()),
                        )
                    } else {
                        (None, Some(remainder.to_owned()))
                    }
                } else if let Some(part) = remainder
                    .strip_prefix('p')
                    .and_then(|part| part.parse::<u64>().ok())
                {
                    (Some(part), None)
                } else {
                    (None, Some(remainder.to_owned()))
                };

                paths
                    .entry(part)
                    .and_modify(|entries: &mut Vec<(PathBuf, Option<String>)>| {
                        entries.push((entry.path(), modifiers.clone()));
                    })
                    .or_insert_with(|| vec![(entry.path(), modifiers)]);
            }
        }

        Ok(paths)
    }

    /// Returns the prefix of the `filename` with the [`ToolOutputPathKind`] removed
    ///
    /// The result is `None` if the output kind was not matched, otherwise the result is `Some`. The
    /// contained prefix can be empty.
    fn strip_output_kind<'a>(&self, filename: &'a str) -> Option<&'a str> {
        match &self.kind {
            ToolOutputPathKind::Out => filename.strip_suffix(".out"),
            ToolOutputPathKind::Log => filename.strip_suffix(".log"),
            ToolOutputPathKind::Data => filename.strip_suffix(".data"),
            ToolOutputPathKind::OldOut => filename.strip_suffix(".out.old"),
            ToolOutputPathKind::OldLog => filename.strip_suffix(".log.old"),
            ToolOutputPathKind::OldData => filename.strip_suffix(".data.old"),
            ToolOutputPathKind::BaseOut(name) => {
                filename.strip_suffix(format!(".out.base@{name}").as_str())
            }
            ToolOutputPathKind::BaseLog(name) => {
                filename.strip_suffix(format!(".log.base@{name}").as_str())
            }
            ToolOutputPathKind::BaseData(name) => {
                filename.strip_suffix(format!(".data.base@{name}").as_str())
            }
            ToolOutputPathKind::Xtree => filename.strip_suffix(".xtree"),
            ToolOutputPathKind::OldXtree => filename.strip_suffix(".xtree.old"),
            ToolOutputPathKind::BaseXtree(name) => {
                filename.strip_suffix(format!(".xtree.base@{name}").as_str())
            }
            ToolOutputPathKind::Xleak => filename.strip_suffix(".xleak"),
            ToolOutputPathKind::OldXleak => filename.strip_suffix(".xleak.old"),
            ToolOutputPathKind::BaseXleak(name) => {
                filename.strip_suffix(format!(".xleak.base@{name}").as_str())
            }
        }
    }

    /// Sanitize file names for a specific tool.
    ///
    /// Dispatches to `PathSanitizer::sanitize`, for more details see there.
    pub fn sanitize(&self) -> Result<()> {
        PathSanitizer::new(self).sanitize()
    }
}

impl<'a> PathSanitizer<'a> {
    fn new(output_path: &'a ToolOutputPath) -> Self {
        Self { output_path }
    }

    fn for_each_match_do(
        &self,
        regex: &Regex,
        mut apply: impl FnMut(DirEntry, Captures<'_>) -> Result<()>,
    ) -> Result<()> {
        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            let Some(haystack) = self.strip_prefix(&file_name) else {
                continue;
            };

            if let Some(caps) = regex.captures(haystack) {
                if entry.metadata()?.size() == 0 {
                    std::fs::remove_file(entry.path())?;
                    continue;
                }

                apply(entry, caps)?;
            }
        }

        Ok(())
    }

    fn sanitizable_baseline(caps: &Captures<'_>) -> SanitizableBaseline {
        let Some(base) = caps.name("base") else {
            return SanitizableBaseline::NoBaseline;
        };

        if base.as_str() == ".old" {
            SanitizableBaseline::OldBaseline
        } else {
            SanitizableBaseline::Baseline(base.as_str().to_owned())
        }
    }

    /// Sanitize callgrind output file names
    ///
    /// This method will remove empty files which are occasionally produced by callgrind and only
    /// cause problems in the parser. The files are renamed from the callgrind file naming scheme to
    /// ours which is clearer and easier to handle.
    ///
    /// The information about pids, parts and threads is obtained by parsing the header from the
    /// callgrind output files instead of relying on the sometimes flaky file names produced by
    /// `callgrind`. The header is around 10-20 lines, so this method should be still sufficiently
    /// fast. Additionally, `callgrind` might change the naming scheme of its files, so using the
    /// headers makes us more independent of a specific valgrind/callgrind version.
    fn sanitize_callgrind(&self) -> Result<()> {
        type Groups = HashMap<
            CallgrindTypeKey,
            HashMap<Option<u32>, HashMap<Option<usize>, Vec<OriginalCallgrindFile>>>,
        >;

        // To figure out if there are multiple pids/parts/threads present, it's necessary to group
        // the files in this map. The order doesn't matter since we only rename the original file
        // names, which doesn't need to follow a specific order.
        //
        // At first, we group by (out|log), then base, then pid and then by part in different
        // hashmaps.
        let mut groups: Groups = HashMap::new();

        self.for_each_match_do(&CALLGRIND_ORIG_FILENAME_RE, |entry, caps| {
            let base = match Self::sanitizable_baseline(&caps) {
                SanitizableBaseline::Baseline(base) => Some(base),
                SanitizableBaseline::NoBaseline => None,
                SanitizableBaseline::OldBaseline => return Ok(()),
            };

            let output_type = caps
                .name("type")
                .expect("A out|log type should be present")
                .as_str()
                .to_owned();

            let (pid, thread, part) = if output_type == ".out" {
                let properties = callgrind::parser::parse_header(
                    &mut BufReader::new(File::open(entry.path())?).lines(),
                )?;

                #[expect(
                    clippy::cast_sign_loss,
                    reason = "The i32 pid is historical and casting to u32 is safe"
                )]
                (
                    properties.pid.map(|p| p as u32),
                    properties.thread,
                    properties.part,
                )
            } else {
                let pid = caps.name("pid").map(|m| {
                    m.as_str()[2..]
                        .parse::<u32>()
                        .expect("The pid from the match should be number")
                });

                // The log files don't expose any information about parts or threads, so these are
                // grouped under the `None` key
                (pid, None, None)
            };

            groups
                .entry(CallgrindTypeKey {
                    output_type,
                    baseline: base,
                })
                .or_default()
                .entry(pid)
                .or_default()
                .entry(thread)
                .or_default()
                .push(OriginalCallgrindFile {
                    path: entry.path(),
                    part,
                });
            Ok(())
        })?;

        for (key, bases) in groups {
            let has_multiple_pids = bases.len() > 1;

            for (pid, threads) in bases {
                let num_threads = threads.len();
                let has_multiple_threads = num_threads > 1;

                for (thread, parts) in threads {
                    let num_parts = parts.len();
                    let has_multiple_parts = num_parts > 1;

                    for original in parts {
                        let mut file_name_builder = SanitizedFileNameBuilder::new(self.prefix());

                        file_name_builder.push_pid(pid, has_multiple_pids);

                        if has_multiple_threads {
                            file_name_builder.push_thread(thread, true, num_threads);

                            if !has_multiple_parts {
                                file_name_builder.push_part(original.part, num_parts);
                            }
                        }

                        if has_multiple_parts {
                            if !has_multiple_threads {
                                file_name_builder.push_thread(thread, true, num_threads);
                            }

                            file_name_builder.push_part(original.part, num_parts);
                        }

                        file_name_builder.push_str(key.output_type.as_str());
                        file_name_builder.push_str(key.baseline.as_deref());

                        let from = &original.path;
                        let to = from.with_file_name(file_name_builder.build());

                        debug!(
                            "Sanitizing callgrind file from '{}' to '{}'",
                            from.display(),
                            to.display()
                        );
                        std::fs::rename(from, to)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Sanitize bbv file names
    ///
    /// The original output files of bb have a `.<number>` suffix if there are multiple threads. We
    /// need the threads as `t<number>` in the modifier part of the final file names.
    ///
    /// For example: (orig -> sanitized)
    ///
    /// If there are multiple threads, the bb output file name doesn't include the first thread:
    ///
    /// `exp-bbv.bench_thread_in_subprocess.548365.bb.out` ->
    /// `exp-bbv.bench_thread_in_subprocess.548365.t1.bb.out`
    ///
    /// `exp-bbv.bench_thread_in_subprocess.548365.bb.out.2` ->
    /// `exp-bbv.bench_thread_in_subprocess.548365.t2.bb.out`
    fn sanitize_bbv(&self) -> Result<()> {
        type Groups = HashMap<BbvTypeKey, HashMap<Option<u32>, Vec<OriginalBbvFile>>>;
        let mut groups: Groups = HashMap::new();

        self.for_each_match_do(&BBV_ORIG_FILENAME_RE, |entry, caps| {
            let base = match Self::sanitizable_baseline(&caps) {
                SanitizableBaseline::Baseline(base) => Some(base),
                SanitizableBaseline::NoBaseline => None,
                SanitizableBaseline::OldBaseline => return Ok(()),
            };

            let output_type = caps.name("type").unwrap().as_str().to_owned();
            let bbv_type = caps.name("bbv_type").map(|m| m.as_str().to_owned());
            let pid = caps.name("pid").map(|p| {
                p.as_str()[2..]
                    .parse::<u32>()
                    .expect("The pid from the regex should be a number")
            });

            let thread = caps.name("thread").map_or(1, |t| {
                t.as_str()[1..]
                    .parse::<usize>()
                    .expect("The thread from the regex should be a number")
            });

            groups
                .entry(BbvTypeKey {
                    output_type,
                    baseline: base,
                    bbv_type,
                })
                .or_default()
                .entry(pid)
                .or_default()
                .push(OriginalBbvFile {
                    path: entry.path(),
                    thread,
                });
            Ok(())
        })?;

        for (key, pids) in groups {
            let has_multiple_pids = pids.len() > 1;

            for (pid, threads) in pids {
                let num_threads = threads.len();
                let has_multiple_threads = num_threads > 1;

                for original in threads {
                    let mut file_name_builder = SanitizedFileNameBuilder::new(self.prefix());

                    file_name_builder.push_pid(pid, has_multiple_pids);

                    if has_multiple_threads
                        && key.bbv_type.as_ref().is_some_and(|b| b.starts_with(".bb"))
                    {
                        file_name_builder.push_thread(original.thread, true, num_threads);
                    }

                    file_name_builder.push_str(key.bbv_type.as_deref());
                    file_name_builder.push_str(key.output_type.as_str());
                    file_name_builder.push_str(key.baseline.as_deref());

                    let from = &original.path;
                    let to = from.with_file_name(file_name_builder.build());

                    debug!(
                        "Sanitizing bbv file from '{}' to '{}'",
                        from.display(),
                        to.display()
                    );
                    std::fs::rename(from, to)?;
                }
            }
        }

        Ok(())
    }

    /// Sanitize file names of all tools if not sanitized by a more specific method
    ///
    /// The pids are removed from the file name if there was only a single process (pid).
    /// Additionally, we check for empty files and remove them.
    fn sanitize_generic(&self) -> Result<()> {
        type Groups = HashMap<GenericTypeKey, Vec<OriginalGenericFile>>;
        let mut groups: Groups = HashMap::new();

        self.for_each_match_do(&GENERIC_ORIG_FILENAME_RE, |entry, caps| {
            let base = match Self::sanitizable_baseline(&caps) {
                SanitizableBaseline::Baseline(base) => Some(base),
                SanitizableBaseline::NoBaseline => None,
                SanitizableBaseline::OldBaseline => return Ok(()),
            };

            let output_type = caps.name("type").unwrap().as_str().to_owned();
            let pid = caps.name("pid").map(|p| {
                p.as_str()[2..]
                    .parse::<u32>()
                    .expect("The pid from the regex should be a number")
            });
            let ext = caps.name("ext").map(|p| p.as_str().to_owned());

            groups
                .entry(GenericTypeKey {
                    output_type,
                    baseline: base,
                })
                .or_default()
                .push(OriginalGenericFile {
                    path: entry.path(),
                    pid,
                    extension: ext,
                });
            Ok(())
        })?;

        for (key, files) in groups {
            let has_multiple_pids = files.len() > 1;
            for original in files {
                let mut file_name_builder = SanitizedFileNameBuilder::new(self.prefix());

                file_name_builder.push_str(original.extension.as_deref());
                file_name_builder.push_pid(original.pid, has_multiple_pids);
                file_name_builder.push_str(key.output_type.as_str());
                file_name_builder.push_str(key.baseline.as_deref());

                let from = &original.path;
                let to = from.with_file_name(file_name_builder.build());

                debug!("Sanitizing from '{}' to '{}'", from.display(), to.display());
                std::fs::rename(from, to)?;
            }
        }

        Ok(())
    }

    /// Sanitize perf output files
    ///
    /// Perf can emit one data, log, or output file per recorded part and modifier. This method
    /// groups matching files by output type, baseline, and modifier so part suffixes are only kept
    /// when a modifier group contains multiple parts.
    fn sanitize_perf(&self) -> Result<()> {
        type Groups = HashMap<PerfTypeKey, HashMap<Option<String>, Vec<OriginalPerfFile>>>;

        // At first, we group by (out|log), then base, then by modifiers, then by part
        let mut groups: Groups = HashMap::new();

        self.for_each_match_do(&PERF_ORIG_FILENAME_RE, |entry, caps| {
            let base = match Self::sanitizable_baseline(&caps) {
                SanitizableBaseline::Baseline(base) => Some(base),
                SanitizableBaseline::NoBaseline => None,
                SanitizableBaseline::OldBaseline => return Ok(()),
            };

            let output_type = caps
                .name("type")
                .expect("A out|log type should be present")
                .as_str()
                .to_owned();
            let ext = caps.name("ext").map(|p| p.as_str().to_owned());
            let part = caps
                .name("part")
                .and_then(|p| p.as_str().strip_prefix(".p")?.parse::<u64>().ok());

            groups
                .entry(PerfTypeKey {
                    output_type,
                    baseline: base,
                })
                .or_default()
                .entry(ext)
                .or_default()
                .push(OriginalPerfFile {
                    path: entry.path(),
                    part,
                });
            Ok(())
        })?;

        for (key, modifiers) in groups {
            for (modifier, parts) in modifiers {
                let num_parts = parts.len();
                let has_multiple_parts = num_parts > 1;

                for original in parts {
                    let mut file_name_builder = SanitizedFileNameBuilder::new(self.prefix());

                    if has_multiple_parts {
                        file_name_builder.push_part(original.part, num_parts);
                    }

                    file_name_builder.push_str(modifier.as_deref());
                    file_name_builder.push_str(key.output_type.as_str());
                    file_name_builder.push_str(key.baseline.as_deref());

                    let from = &original.path;
                    let to = from.with_file_name(file_name_builder.build());

                    debug!(
                        "Sanitizing perf file from '{}' to '{}'",
                        from.display(),
                        to.display()
                    );
                    std::fs::rename(from, to)?;
                }
            }
        }

        Ok(())
    }

    /// Sanitize file names for a specific tool
    ///
    /// Empty files are cleaned up. For more details on a specific tool see the respective
    /// `sanitize_<tool>` method in [`PathSanitizer`]:
    ///
    /// * Callgrind: [`PathSanitizer::sanitize_callgrind`]
    /// * BBV: [`PathSanitizer::sanitize_bbv`]
    /// * perf: [`PathSanitizer::sanitize_perf`]
    /// * All other tools: [`PathSanitizer::sanitize`]
    pub fn sanitize(&self) -> Result<()> {
        match self.tool {
            Tool::Callgrind => self.sanitize_callgrind()?,
            Tool::BBV => self.sanitize_bbv()?,
            Tool::Perf => self.sanitize_perf()?,
            _ => self.sanitize_generic()?,
        }

        Ok(())
    }
}

impl SanitizedFileNameBuilder {
    fn new(prefix: impl Into<String>) -> Self {
        Self(prefix.into())
    }

    fn push_str<'a, T>(&'a mut self, string: T) -> &'a mut Self
    where
        T: Into<Option<&'a str>>,
    {
        if let Some(suffix) = string.into() {
            self.0.push_str(suffix);
        }

        self
    }

    fn push_pid<T>(&mut self, pid: T, has_multiple: bool) -> &mut Self
    where
        T: Into<Option<u32>>,
    {
        if has_multiple && let Some(pid) = pid.into() {
            write!(&mut self.0, ".{pid}").unwrap();
        }

        self
    }

    fn push_thread<T>(&mut self, thread: T, has_multiple: bool, num_threads: usize) -> &mut Self
    where
        T: Into<Option<usize>>,
    {
        if has_multiple && let Some(thread) = thread.into() {
            let width = Self::width(num_threads);
            write!(&mut self.0, ".t{thread:0width$}").unwrap();
        }

        self
    }

    fn push_part<T>(&mut self, part: T, num_parts: usize) -> &mut Self
    where
        T: Into<Option<u64>>,
    {
        if let Some(part) = part.into() {
            let width = Self::width(num_parts);
            write!(&mut self.0, ".p{part:0width$}").unwrap();
        }

        self
    }

    fn width(num: usize) -> usize {
        num.ilog10() as usize + 1
    }

    fn build(self) -> String {
        self.0
    }
}

impl Display for ToolOutputPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.to_path().display()))
    }
}

#[cfg(test)]
mod tests {

    use rstest::rstest;

    use super::*;
    use crate::runner::perf::run::{PERF_CALIBRATION_FILE_MODIFIER, PERF_OVERHEAD_FILE_MODIFIER};

    type ExpectedPath<'a> = (&'a str, Option<&'a str>);
    type ExpectedPart<'a> = (Option<u64>, Vec<ExpectedPath<'a>>);

    #[rstest]
    #[case::skips_when_not_multiple(Some(1234), false, "tool.bench")]
    #[case::skips_none(None, true, "tool.bench")]
    fn test_file_name_builder_push_pid(
        #[case] pid: Option<u32>,
        #[case] multiple: bool,
        #[case] expected: &str,
    ) {
        let mut file_name_builder = SanitizedFileNameBuilder::new("tool.bench");

        file_name_builder.push_pid(pid, multiple);

        assert_eq!(file_name_builder.build(), expected);
    }

    #[rstest]
    #[case::width_1(Some(1), true, 1, "tool.bench.t1")]
    #[case::width_2(Some(1), true, 10, "tool.bench.t01")]
    #[case::width_3(Some(42), true, 100, "tool.bench.t042")]
    #[case::skips_when_not_multiple(Some(1), false, 10, "tool.bench")]
    #[case::skips_none(None, true, 10, "tool.bench")]
    fn test_file_name_builder_push_thread(
        #[case] thread: Option<usize>,
        #[case] multiple: bool,
        #[case] group_len: usize,
        #[case] expected: &str,
    ) {
        let mut file_name_builder = SanitizedFileNameBuilder::new("tool.bench");

        file_name_builder.push_thread(thread, multiple, group_len);

        assert_eq!(file_name_builder.build(), expected);
    }

    #[rstest]
    #[case::width_1(Some(1), 1, "tool.bench.p1")]
    #[case::width_2(Some(1), 10, "tool.bench.p01")]
    #[case::width_3(Some(42), 100, "tool.bench.p042")]
    #[case::skips_none(None, 10, "tool.bench")]
    fn test_file_name_builder_push_part(
        #[case] part: Option<u64>,
        #[case] group_len: usize,
        #[case] expected: &str,
    ) {
        let mut file_name_builder = SanitizedFileNameBuilder::new("tool.bench");

        file_name_builder.push_part(part, group_len);

        assert_eq!(file_name_builder.build(), expected);
    }

    #[rstest]
    #[case::string(Some(".bb"), Some(".base@default"), "tool.bench.bb.out.base@default")]
    #[case::none(None, None, "tool.bench.out")]
    fn test_file_name_builder_push_str_and_optional(
        #[case] modifier: Option<&str>,
        #[case] baseline: Option<&str>,
        #[case] expected: &str,
    ) {
        let mut file_name_builder = SanitizedFileNameBuilder::new("tool.bench");

        file_name_builder
            .push_str(modifier)
            .push_str(".out")
            .push_str(baseline);

        assert_eq!(file_name_builder.build(), expected);
    }

    #[rstest]
    #[case::all_segments(
        Some(1234),
        Some(1),
        Some(2),
        Some(".cal"),
        ".out",
        Some(".base@default"),
        "tool.bench.1234.t01.p02.cal.out.base@default"
    )]
    fn test_file_name_builder_full_sequence(
        #[case] pid: Option<u32>,
        #[case] thread: Option<usize>,
        #[case] part: Option<u64>,
        #[case] modifier: Option<&str>,
        #[case] output_kind: &str,
        #[case] baseline: Option<&str>,
        #[case] expected: &str,
    ) {
        let mut file_name_builder = SanitizedFileNameBuilder::new("tool.bench");

        file_name_builder
            .push_pid(pid, true)
            .push_thread(thread, true, 10)
            .push_part(part, 10)
            .push_str(modifier)
            .push_str(output_kind)
            .push_str(baseline);

        assert_eq!(file_name_builder.build(), expected);
    }

    #[rstest]
    #[case::out(".out")]
    #[case::out_with_pid(".out.#1234")]
    #[case::out_with_number(".out.1")]
    #[case::out_with_some(".out.some")]
    #[case::out_base(".out.base@default")]
    #[case::out_base_with_pid(".out.base@default.#1234")]
    #[case::out_base_with_number(".out.base@default.1")]
    #[case::out_base_with_some(".out.base@default.some")]
    #[case::log(".log")]
    #[case::log_with_pid(".log.#1234")]
    #[case::log_base(".log.base@default")]
    #[case::log_base_with_pid(".log.base@default.#1234")]
    fn test_generic_filename_regex(#[case] haystack: &str) {
        assert!(GENERIC_ORIG_FILENAME_RE.is_match(haystack));
    }

    #[rstest]
    #[case::out(".out")]
    #[case::out_with_pid(".out.#1234")]
    #[case::out_with_part(".out.1")]
    #[case::out_with_thread(".out-01")]
    #[case::out_with_part_and_thread(".out.1-01")]
    #[case::out_base(".out.base@default")]
    #[case::out_base_with_pid(".out.base@default.#1234")]
    #[case::out_base_with_part(".out.base@default.1")]
    #[case::out_base_with_thread(".out.base@default-01")]
    #[case::out_base_with_part_and_thread(".out.base@default.1-01")]
    #[case::log(".log")]
    #[case::log_with_pid(".log.#1234")]
    #[case::log_base(".log.base@default")]
    #[case::log_base_with_pid(".log.base@default.#1234")]
    fn test_callgrind_filename_regex(#[case] haystack: &str) {
        assert!(CALLGRIND_ORIG_FILENAME_RE.is_match(haystack));
    }

    #[rstest]
    #[case::bb_out(".out.bb")]
    #[case::bb_out_with_pid(".out.bb.#1234")]
    #[case::bb_out_with_pid_and_thread(".out.bb.#1234.1")]
    #[case::bb_out_with_thread(".out.bb.1")]
    #[case::pc_out(".out.pc")]
    #[case::log(".log")]
    #[case::log_with_pid(".log.#1234")]
    fn test_bbv_filename_regex(#[case] haystack: &str) {
        assert!(BBV_ORIG_FILENAME_RE.is_match(haystack));
    }

    #[rstest]
    #[case::out(".out")]
    #[case::out_with_pid(".out")]
    #[case::out_with_part(".out.p1")]
    #[case::out_with_some(".out.some")]
    #[case::out_with_some_and_part(".out.p1.some")]
    #[case::out_base_with_multiple_ext(".out.p1.some.more")]
    #[case::out_base(".out.base@default")]
    #[case::out_base_with_part(".out.base@default.p1")]
    #[case::out_base_with_some_and_part(".out.base@default.p1.some")]
    #[case::out_base_with_multiple_ext(".out.base@default.p1.some.more")]
    #[case::log(".log")]
    #[case::log_with_part(".log.p1")]
    #[case::log_base(".log.base@default")]
    #[case::log_base_with_part(".log.base@default.p1")]
    fn test_perf_filename_regex(#[case] haystack: &str) {
        assert!(PERF_ORIG_FILENAME_RE.is_match(haystack));
    }

    #[rstest]
    #[case::plain_out(
        ToolOutputPathKind::Out,
        &["perf.function.bench.out"],
        &[(None, vec![("perf.function.bench.out", None)])]
    )]
    #[case::plain_log(
        ToolOutputPathKind::Log,
        &["perf.function.bench.log"],
        &[(None, vec![("perf.function.bench.log", None)])]
    )]
    #[case::single_part_out(
        ToolOutputPathKind::Out,
        &["perf.function.bench.p1.out"],
        &[(Some(1), vec![("perf.function.bench.p1.out", None)])]
    )]
    #[case::single_part_log(
        ToolOutputPathKind::Log,
        &["perf.function.bench.p1.log"],
        &[(Some(1), vec![("perf.function.bench.p1.log", None)])]
    )]
    #[case::part_with_cal(
        ToolOutputPathKind::Out,
        &["perf.function.bench.p1.cal.out"],
        &[(Some(1), vec![("perf.function.bench.p1.cal.out", Some(PERF_CALIBRATION_FILE_MODIFIER))])]
    )]
    #[case::part_with_overhead(
        ToolOutputPathKind::Out,
        &["perf.function.bench.p1.overhead.out"],
        &[(
                Some(1),
                vec![("perf.function.bench.p1.overhead.out", Some(PERF_OVERHEAD_FILE_MODIFIER))]
        )]
    )]
    #[case::multiple_parts_with_adjustments(
        ToolOutputPathKind::Out,
        &[
            "perf.function.bench.p1.out",
            "perf.function.bench.p1.cal.out",
            "perf.function.bench.p1.overhead.out",
            "perf.function.bench.p2.out",
            "perf.function.bench.p2.cal.out",
        ],
        &[
            (
                Some(1),
                vec![
                    ("perf.function.bench.p1.out", None),
                    ("perf.function.bench.p1.cal.out", Some(PERF_CALIBRATION_FILE_MODIFIER)),
                    ("perf.function.bench.p1.overhead.out", Some(PERF_OVERHEAD_FILE_MODIFIER)),
                ],
            ),
            (
                Some(2),
                vec![
                    ("perf.function.bench.p2.out", None),
                    ("perf.function.bench.p2.cal.out", Some(PERF_CALIBRATION_FILE_MODIFIER)),
                ],
            ),
        ]
    )]
    fn test_sanitized_paths_by_part(
        #[case] kind: ToolOutputPathKind,
        #[case] files: &[&str],
        #[case] expected: &[ExpectedPart<'_>],
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = ToolOutputPath::new(
            kind,
            Tool::Perf,
            &BaselineKind::Old,
            temp_dir.path(),
            &ModulePath::new("module"),
            "function.bench",
            false,
        )
        .unwrap();
        output_path.init().unwrap();

        for file in files {
            std::fs::write(output_path.dir.join(file), "something").unwrap();
        }

        let actual = output_path.sanitized_paths_by_part().unwrap();

        let mut actual = actual
            .into_iter()
            .map(|(part, paths)| {
                let mut paths = paths
                    .into_iter()
                    .map(|(path, modifier)| {
                        (
                            path.file_name().unwrap().to_string_lossy().to_string(),
                            modifier,
                        )
                    })
                    .collect::<Vec<_>>();
                paths.sort();

                (part, paths)
            })
            .collect::<Vec<_>>();
        actual.sort_by_key(|(part, _)| *part);

        let mut expected = expected
            .iter()
            .map(|(part, paths)| {
                let mut paths = paths
                    .iter()
                    .map(|(path, modifier)| ((*path).to_owned(), modifier.map(str::to_owned)))
                    .collect::<Vec<_>>();
                paths.sort();

                (*part, paths)
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(|(part, _)| *part);

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::out(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.out",
        "callgrind.bench_thread_in_subprocess.two.log"
    )]
    #[case::out_with_modifier(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.some.out",
        "callgrind.bench_thread_in_subprocess.two.some.log"
    )]
    #[case::out_old(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.out.old",
        "callgrind.bench_thread_in_subprocess.two.log.old"
    )]
    #[case::out_old_with_modifier(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.some.out.old",
        "callgrind.bench_thread_in_subprocess.two.some.log.old"
    )]
    #[case::pid_out(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.out",
        "callgrind.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::pid_out_with_modifier(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.some.123.out",
        "callgrind.bench_thread_in_subprocess.two.some.123.log"
    )]
    #[case::pid_tid_out(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.t1.out",
        "callgrind.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::pid_tid_part_out(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.t1.p2.out",
        "callgrind.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::pid_out_old(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.out.old",
        "callgrind.bench_thread_in_subprocess.two.123.log.old"
    )]
    #[case::pid_tid_part_out_old(
        Tool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.t1.p2.out.old",
        "callgrind.bench_thread_in_subprocess.two.123.log.old"
    )]
    #[case::bb_out(
        Tool::BBV,
        "exp-bbv.bench_thread_in_subprocess.two.bb.out",
        "exp-bbv.bench_thread_in_subprocess.two.log"
    )]
    #[case::bb_pid_out(
        Tool::BBV,
        "exp-bbv.bench_thread_in_subprocess.two.123.bb.out",
        "exp-bbv.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::bb_pid_tid_out(
        Tool::BBV,
        "exp-bbv.bench_thread_in_subprocess.two.123.t1.bb.out",
        "exp-bbv.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::xtree(
        Tool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xtree",
        "memcheck.bench_thread_in_subprocess.two.log"
    )]
    #[case::xtree_old(
        Tool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xtree.old",
        "memcheck.bench_thread_in_subprocess.two.log.old"
    )]
    #[case::xtree_pid(
        Tool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.123.xtree",
        "memcheck.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::xleak(
        Tool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xleak",
        "memcheck.bench_thread_in_subprocess.two.log"
    )]
    #[case::xleak_old(
        Tool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xleak.old",
        "memcheck.bench_thread_in_subprocess.two.log.old"
    )]
    #[case::xleak_pid(
        Tool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.123.xleak",
        "memcheck.bench_thread_in_subprocess.two.123.log"
    )]
    fn test_tool_output_path_log_path_of(
        #[case] tool: Tool,
        #[case] input: PathBuf,
        #[case] expected: PathBuf,
    ) {
        let output_path = ToolOutputPath::new(
            ToolOutputPathKind::Out,
            tool,
            &BaselineKind::Old,
            &PathBuf::from("/root"),
            &ModulePath::new("hello::world"),
            "bench_thread_in_subprocess.two",
            false,
        )
        .unwrap();
        let expected = output_path.dir.join(expected);
        let actual = output_path
            .log_path_of(&output_path.dir.join(input))
            .unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_tool_output_path_log_path_of_when_log_then_same() {
        let output_path = ToolOutputPath::new(
            ToolOutputPathKind::Log,
            Tool::Callgrind,
            &BaselineKind::Old,
            &PathBuf::from("/root"),
            &ModulePath::new("hello::world"),
            "bench_thread_in_subprocess.two",
            false,
        )
        .unwrap();
        let path = PathBuf::from(
            "/root/hello/world/bench_thread_in_subprocess.two/callgrind.\
             bench_thread_in_subprocess.two.log",
        );

        assert_eq!(output_path.log_path_of(&path), Some(path));
    }

    #[test]
    fn test_tool_output_path_log_path_of_when_not_in_dir_then_none() {
        let output_path = ToolOutputPath::new(
            ToolOutputPathKind::Out,
            Tool::Callgrind,
            &BaselineKind::Old,
            &PathBuf::from("/root"),
            &ModulePath::new("hello::world"),
            "bench_thread_in_subprocess.two",
            false,
        )
        .unwrap();

        assert!(
            output_path
                .log_path_of(&PathBuf::from(
                    "/root/not/here/bench_thread_in_subprocess.two/callgrind.\
                     bench_thread_in_subprocess.two.out"
                ))
                .is_none()
        );
    }
}
