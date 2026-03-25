//! The module containing the [`ToolOutputPath`] and other related elements

use std::collections::HashMap;
use std::fmt::{Display, Write as FmtWrite};
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use lazy_static::lazy_static;
use log::{debug, log_enabled};
use regex::Regex;
use tempfile::{Builder, TempDir};

use crate::api::ValgrindTool;
use crate::runner::callgrind::parser::parse_header;
use crate::runner::common::ModulePath;
use crate::runner::summary::{BaselineKind, BaselineName};
use crate::util::truncate_str_utf8;

lazy_static! {
    // This regex matches the original file name without the prefix as it is created by callgrind.
    // The baseline <name> (base@<name>) can only consist of ascii and underscore characters.
    // Flamegraph files are ignored by this regex
    //
    // Note callgrind doesn't support xtree, xleak files
    static ref CALLGRIND_ORIG_FILENAME_RE: Regex = Regex::new(
        concat!(
            "^(?<type>[.](out|log))(?<base>[.](old|base@[^.-]+))?",
            "(?<pid>[.][#][0-9]+)?(?<part>[.][0-9]+)?(?<thread>-[0-9]+)?$"
        )
    )
    .expect("Regex should compile");

    /// This regex matches the original file name without the prefix as it is created by bbv
    ///
    /// Note bbv doesn't support xtree, xleak files
    static ref BBV_ORIG_FILENAME_RE: Regex = Regex::new(
        concat!(
            "^(?<type>[.](?:out|log))(?<base>[.](old|base@[^.]+))?",
            "(?<bbv_type>[.](?:bb|pc))?(?<pid>[.][#][0-9]+)?(?<thread>[.][0-9]+)?$"
        )
    )
    .expect("Regex should compile");

    /// This regex matches the original file name without the prefix as it is created by all tools
    /// other than callgrind and bbv.
    static ref GENERIC_ORIG_FILENAME_RE: Regex = Regex::new(
        "^(?<type>[.](?:out|log|xtree|xleak))(?<base>[.](old|base@[^.]+))?(?<pid>[.][#][0-9]+)?$"
    )
    .expect("Regex should compile");

    static ref REAL_FILENAME_RE: Regex = Regex::new(
        concat!(
            "^(?:[.](?<pid>[0-9]+))?(?:[.]t(?<tid>[0-9]+))?(?:[.]p(?<part>[0-9]+))?",
            "(?:[.](?<bbv>bb|pc))?(?:[.](?<type>out|log|xtree|xleak))",
            "(?:[.](?<base>old|base@[^.]+))?$"
        )
    )
    .expect("Regex should compile");
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

/// The tool specific output path(s)
///
/// In the presence of a temporary directory, the temporary directory is assumed to be the output
/// path for any new files from the valgrind tools and files in it are returned by methods like
/// [`ToolOutputPath::real_paths`]. Otherwise the benchmark directory contains the new and "old"
/// files of previous benchmark runs. The temporary files need to be transferred to the benchmark
/// directory manually for example with [`ToolOutputPath::copy_temp`] and doesn't happen on drop.
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
    pub tool: ValgrindTool,
}

impl ToolOutputPath {
    /// Create a new `ToolOutputPath`.
    ///
    /// The `base_dir` is supposed to be the same as [`crate::runner::meta::Metadata::target_dir`].
    /// The `name` is supposed to be the name of the benchmark function. If a benchmark id is
    /// present join both with a dot as separator to get the final `name`.
    pub fn new(
        kind: ToolOutputPathKind,
        tool: ValgrindTool,
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
        tool: ValgrindTool,
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

    /// Remove the files of this output path
    pub fn clear(&self) -> Result<()> {
        for entry in self.real_paths_in(&self.dir)? {
            debug!("Clearing '{}'", entry.display());
            std::fs::remove_file(&entry).with_context(|| {
                format!("Failed to remove benchmark file: '{}'", entry.display())
            })?;
        }

        self.clear_temp_files(false)
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

    /// Remove the old or base files and rename the present files to "old" files
    pub fn shift(&self) -> Result<()> {
        match self.baseline_kind {
            BaselineKind::Old => {
                self.to_base_path().clear()?;
                for entry in self.real_paths_in(&self.dir)? {
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

    /// Copies the files in the temporary directory to the benchmark directory
    ///
    /// If there is no temporary directory then this method does nothing.
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

    /// Moves the files of the temporary directory to the benchmark directory
    ///
    /// Like [`Self::copy_temp`] this method does nothing if there is no temporary directory. This
    /// method does not delete the temporary directory itself and is still usable if required.
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
            | ToolOutputPathKind::BaseXtree(name)
            | ToolOutputPathKind::BaseXleak(name) => Some(BaselineName(name.clone())),
            _ => None,
        }
    }

    /// Returns `true` if a real file of this output path exists.
    pub fn exists(&self) -> bool {
        self.real_paths().is_ok_and(|p| !p.is_empty())
    }

    /// Returns `true` if there are multiple real files of this output path.
    pub fn is_multiple(&self) -> bool {
        self.real_paths().is_ok_and(|p| p.len() > 1)
    }

    /// Return `true` if this output path is an old or baseline path
    pub fn is_base_path(&self) -> bool {
        match self.kind {
            ToolOutputPathKind::Out
            | ToolOutputPathKind::Log
            | ToolOutputPathKind::Xtree
            | ToolOutputPathKind::Xleak => false,
            ToolOutputPathKind::OldOut
            | ToolOutputPathKind::BaseOut(_)
            | ToolOutputPathKind::OldLog
            | ToolOutputPathKind::BaseLog(_)
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

    /// Convert this tool output path to the output of another tool output path
    ///
    /// A tool with no `*.out` file is log-file based. If the other tool is a out-file based tool
    /// the [`ToolOutputPathKind`] will be converted and vice-versa. The "old" (base) type (a tool
    /// output converted with [`ToolOutputPath::to_base_path`]) will be converted to a new
    /// `ToolOutputPath`.
    #[must_use]
    pub fn to_tool_output(&self, tool: ValgrindTool) -> Self {
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
                | ToolOutputPathKind::Xleak
                | ToolOutputPathKind::OldXleak
                | ToolOutputPathKind::Xtree
                | ToolOutputPathKind::OldXtree => ToolOutputPathKind::Log,
                ToolOutputPathKind::BaseOut(name)
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
                | ToolOutputPathKind::Xleak
                | ToolOutputPathKind::OldXleak
                | ToolOutputPathKind::Log
                | ToolOutputPathKind::OldLog => ToolOutputPathKind::Xtree,
                ToolOutputPathKind::BaseOut(name)
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
            let caps = REAL_FILENAME_RE.captures(suffix)?;
            if let Some(kind) = caps.name("type") {
                match kind.as_str() {
                    "out" | "xtree" | "xleak" => {
                        let mut string = self.prefix();
                        for s in [
                            caps.name("pid").map(|c| format!(".{}", c.as_str())),
                            Some(".log".to_owned()),
                            caps.name("base").map(|c| format!(".{}", c.as_str())),
                        ]
                        .iter()
                        .filter_map(|s| s.as_ref())
                        {
                            string.push_str(s);
                        }

                        let dir = temp_path.unwrap_or(&self.dir);
                        return Some(dir.join(string));
                    }
                    _ => return Some(path.to_owned()),
                }
            }
        }

        None
    }

    /// If the [`log::Level`] matches dump the content of all output files into the `writer`
    pub fn dump_log<W>(&self, log_level: log::Level, writer: &mut W) -> Result<()>
    where
        W: Write,
    {
        if log_enabled!(log_level) {
            for path in self.real_paths()? {
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
            (ToolOutputPathKind::OldOut, true) => "out.old".to_owned(),
            (ToolOutputPathKind::OldOut, false) => format!("out.old.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::OldLog, true) => "log.old".to_owned(),
            (ToolOutputPathKind::OldLog, false) => format!("log.old.{}", self.modifiers.join(".")),
            (ToolOutputPathKind::BaseLog(name), true) => {
                format!("log.base@{name}")
            }
            (ToolOutputPathKind::BaseLog(name), false) => {
                format!("log.base@{name}.{}", self.modifiers.join("."))
            }
            (ToolOutputPathKind::BaseOut(name), true) => format!("out.base@{name}"),
            (ToolOutputPathKind::BaseOut(name), false) => {
                format!("out.base@{name}.{}", self.modifiers.join("."))
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

    /// Return the unexpanded path usable as input for `--callgrind-out-file`, ...
    ///
    /// The path returned by this method does not necessarily have to exist and can include
    /// modifiers like `%p`. Use [`Self::real_paths`] to get the real and existing (possibly
    /// multiple) paths to the output files of the respective tool.
    pub fn to_path(&self) -> PathBuf {
        self.dest_dir().join(format!(
            "{}.{}.{}",
            self.tool.id(),
            self.name,
            self.extension()
        ))
    }

    /// Walk the benchmark directory (non-recursive)
    pub fn walk_dir(&self, dir: Option<&Path>) -> Result<impl Iterator<Item = DirEntry>> {
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

    /// Returns the `real` paths of a tool's output files.
    ///
    /// A tool can have many output files so [`Self::to_path`] is not enough
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    pub fn real_paths(&self) -> Result<Vec<PathBuf>> {
        let dir = if self.is_base_path() {
            &self.dir
        } else {
            self.dest_dir()
        };
        self.real_paths_in(dir)
    }

    /// Returns the `real` paths of a tool's output files.
    ///
    /// A tool can have many output files so [`Self::to_path`] is not enough
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    pub fn real_paths_in(&self, dir: &Path) -> Result<Vec<PathBuf>> {
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
                    ToolOutputPathKind::OldOut => suffix.ends_with(".out.old"),
                    ToolOutputPathKind::OldLog => suffix.ends_with(".log.old"),
                    ToolOutputPathKind::BaseLog(name) => {
                        suffix.ends_with(format!(".log.base@{name}").as_str())
                    }
                    ToolOutputPathKind::BaseOut(name) => {
                        suffix.ends_with(format!(".out.base@{name}").as_str())
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

    /// Returns the real paths with their respective modifiers if present.
    pub fn real_paths_with_modifier(&self) -> Result<Vec<(PathBuf, Option<String>)>> {
        let mut paths = vec![];
        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Silently ignore all paths which don't follow this scheme, for example
            // (`summary.json`)
            if let Some(suffix) = self.strip_prefix(&file_name) {
                let modifiers = match &self.kind {
                    ToolOutputPathKind::Out => suffix.strip_suffix(".out"),
                    ToolOutputPathKind::Log => suffix.strip_suffix(".log"),
                    ToolOutputPathKind::OldOut => suffix.strip_suffix(".out.old"),
                    ToolOutputPathKind::OldLog => suffix.strip_suffix(".log.old"),
                    ToolOutputPathKind::BaseLog(name) => {
                        suffix.strip_suffix(format!(".log.base@{name}").as_str())
                    }
                    ToolOutputPathKind::BaseOut(name) => {
                        suffix.strip_suffix(format!(".out.base@{name}").as_str())
                    }
                    ToolOutputPathKind::Xtree => suffix.strip_suffix(".xtree"),
                    ToolOutputPathKind::OldXtree => suffix.strip_suffix(".xtree.old"),
                    ToolOutputPathKind::BaseXtree(name) => {
                        suffix.strip_suffix(format!(".xtree.base@{name}").as_str())
                    }
                    ToolOutputPathKind::Xleak => suffix.strip_suffix(".xleak"),
                    ToolOutputPathKind::OldXleak => suffix.strip_suffix(".xleak.old"),
                    ToolOutputPathKind::BaseXleak(name) => {
                        suffix.strip_suffix(format!(".xleak.base@{name}").as_str())
                    }
                };

                paths.push((
                    entry.path(),
                    modifiers.and_then(|s| (!s.is_empty()).then(|| s.to_owned())),
                ));
            }
        }
        Ok(paths)
    }

    /// Sanitize callgrind output file names
    ///
    /// This method will remove empty files which are occasionally produced by callgrind and only
    /// cause problems in the parser. The files are renamed from the callgrind file naming scheme to
    /// ours which is easier to handle.
    ///
    /// The information about pids, parts and threads is obtained by parsing the header from the
    /// callgrind output files instead of relying on the sometimes flaky file names produced by
    /// `callgrind`. The header is around 10-20 lines, so this method should be still sufficiently
    /// fast. Additionally, `callgrind` might change the naming scheme of its files, so using the
    /// headers makes us more independent of a specific valgrind/callgrind version.
    #[allow(clippy::too_many_lines)]
    pub fn sanitize_callgrind(&self) -> Result<()> {
        // path, part
        type Grouped = (PathBuf, Option<u64>);
        // base (i.e. base@default) => pid => thread => vec: path, part
        type Group =
            HashMap<Option<String>, HashMap<Option<i32>, HashMap<Option<usize>, Vec<Grouped>>>>;

        // To figure out if there are multiple pids/parts/threads present, it's necessary to group
        // the files in this map. The order doesn't matter since we only rename the original file
        // names, which doesn't need to follow a specific order.
        //
        // At first, we group by (out|log), then base, then pid and then by part in different
        // hashmaps. The threads are grouped in a vector.
        let mut groups: HashMap<String, Group> = HashMap::new();

        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            let Some(haystack) = self.strip_prefix(&file_name) else {
                continue;
            };

            if let Some(caps) = CALLGRIND_ORIG_FILENAME_RE.captures(haystack) {
                // Callgrind sometimes creates empty files for no reason. We clean them
                // up here
                if entry.metadata()?.size() == 0 {
                    std::fs::remove_file(entry.path())?;
                    continue;
                }

                // We don't sanitize old files. It's not needed if the new files are always
                // sanitized. However, we do sanitize `base@<name>` file names.
                let base = if let Some(base) = caps.name("base") {
                    if base.as_str() == ".old" {
                        continue;
                    }

                    Some(base.as_str().to_owned())
                } else {
                    None
                };

                let out_type = caps
                    .name("type")
                    .expect("A out|log type should be present")
                    .as_str();

                if out_type == ".out" {
                    let properties = parse_header(
                        &mut BufReader::new(File::open(entry.path())?)
                            .lines()
                            .map(Result::unwrap),
                    )?;
                    if let Some(bases) = groups.get_mut(out_type) {
                        if let Some(pids) = bases.get_mut(&base) {
                            if let Some(threads) = pids.get_mut(&properties.pid) {
                                if let Some(parts) = threads.get_mut(&properties.thread) {
                                    parts.push((entry.path(), properties.part));
                                } else {
                                    threads.insert(
                                        properties.thread,
                                        vec![(entry.path(), properties.part)],
                                    );
                                }
                            } else {
                                pids.insert(
                                    properties.pid,
                                    HashMap::from([(
                                        properties.thread,
                                        vec![(entry.path(), properties.part)],
                                    )]),
                                );
                            }
                        } else {
                            bases.insert(
                                base.clone(),
                                HashMap::from([(
                                    properties.pid,
                                    HashMap::from([(
                                        properties.thread,
                                        vec![(entry.path(), properties.part)],
                                    )]),
                                )]),
                            );
                        }
                    } else {
                        groups.insert(
                            out_type.to_owned(),
                            HashMap::from([(
                                base.clone(),
                                HashMap::from([(
                                    properties.pid,
                                    HashMap::from([(
                                        properties.thread,
                                        vec![(entry.path(), properties.part)],
                                    )]),
                                )]),
                            )]),
                        );
                    }
                } else {
                    let pid = caps.name("pid").map(|m| {
                        m.as_str()[2..]
                            .parse::<i32>()
                            .expect("The pid from the match should be number")
                    });

                    // The log files don't expose any information about parts or threads, so
                    // these are grouped under the `None` key
                    if let Some(bases) = groups.get_mut(out_type) {
                        if let Some(pids) = bases.get_mut(&base) {
                            if let Some(threads) = pids.get_mut(&pid) {
                                if let Some(parts) = threads.get_mut(&None) {
                                    parts.push((entry.path(), None));
                                } else {
                                    threads.insert(None, vec![(entry.path(), None)]);
                                }
                            } else {
                                pids.insert(
                                    pid,
                                    HashMap::from([(None, vec![(entry.path(), None)])]),
                                );
                            }
                        } else {
                            bases.insert(
                                base.clone(),
                                HashMap::from([(
                                    pid,
                                    HashMap::from([(None, vec![(entry.path(), None)])]),
                                )]),
                            );
                        }
                    } else {
                        groups.insert(
                            out_type.to_owned(),
                            HashMap::from([(
                                base.clone(),
                                HashMap::from([(
                                    pid,
                                    HashMap::from([(None, vec![(entry.path(), None)])]),
                                )]),
                            )]),
                        );
                    }
                }
            }
        }

        for (out_type, types) in groups {
            for (base, bases) in types {
                let multiple_pids = bases.len() > 1;

                for (pid, threads) in bases {
                    let multiple_threads = threads.len() > 1;

                    for (thread, parts) in &threads {
                        let multiple_parts = parts.len() > 1;

                        for (orig_path, part) in parts {
                            let mut new_file_name = self.prefix();

                            if multiple_pids {
                                if let Some(pid) = pid {
                                    write!(new_file_name, ".{pid}").unwrap();
                                }
                            }

                            if multiple_threads {
                                if let Some(thread) = thread {
                                    let width = threads.len().ilog10() as usize + 1;
                                    write!(new_file_name, ".t{thread:0width$}").unwrap();
                                }

                                if !multiple_parts {
                                    if let Some(part) = part {
                                        let width = parts.len().ilog10() as usize + 1;
                                        write!(new_file_name, ".p{part:0width$}").unwrap();
                                    }
                                }
                            }

                            if multiple_parts {
                                if !multiple_threads {
                                    if let Some(thread) = thread {
                                        let width = threads.len().ilog10() as usize + 1;
                                        write!(new_file_name, ".t{thread:0width$}").unwrap();
                                    }
                                }

                                if let Some(part) = part {
                                    let width = parts.len().ilog10() as usize + 1;
                                    write!(new_file_name, ".p{part:0width$}").unwrap();
                                }
                            }

                            new_file_name.push_str(&out_type);
                            if let Some(base) = &base {
                                new_file_name.push_str(base);
                            }

                            let from = orig_path;
                            let to = from.with_file_name(new_file_name);

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
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    #[allow(clippy::too_many_lines)]
    pub fn sanitize_bbv(&self) -> Result<()> {
        // path, thread,
        type Grouped = (PathBuf, String);
        // key: bbv_type => key: pid
        type Group =
            HashMap<Option<String>, HashMap<Option<String>, HashMap<Option<String>, Vec<Grouped>>>>;

        // key: .(out|log)
        let mut groups: HashMap<String, Group> = HashMap::new();
        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            let Some(haystack) = self.strip_prefix(&file_name) else {
                continue;
            };

            if let Some(caps) = BBV_ORIG_FILENAME_RE.captures(haystack) {
                if entry.metadata()?.size() == 0 {
                    std::fs::remove_file(entry.path())?;
                    continue;
                }

                // Don't sanitize old files.
                let base = if let Some(base) = caps.name("base") {
                    if base.as_str() == ".old" {
                        continue;
                    }

                    Some(base.as_str().to_owned())
                } else {
                    None
                };

                let out_type = caps.name("type").unwrap().as_str();
                let bbv_type = caps.name("bbv_type").map(|m| m.as_str().to_owned());
                let pid = caps.name("pid").map(|p| format!(".{}", &p.as_str()[2..]));

                let thread = caps
                    .name("thread")
                    .map_or_else(|| ".1".to_owned(), |t| t.as_str().to_owned());

                if let Some(bases) = groups.get_mut(out_type) {
                    if let Some(bbv_types) = bases.get_mut(&base) {
                        if let Some(pids) = bbv_types.get_mut(&bbv_type) {
                            if let Some(threads) = pids.get_mut(&pid) {
                                threads.push((entry.path(), thread));
                            } else {
                                pids.insert(pid, vec![(entry.path(), thread)]);
                            }
                        } else {
                            bbv_types.insert(
                                bbv_type.clone(),
                                HashMap::from([(pid, vec![(entry.path(), thread)])]),
                            );
                        }
                    } else {
                        bases.insert(
                            base.clone(),
                            HashMap::from([(
                                bbv_type.clone(),
                                HashMap::from([(pid, vec![(entry.path(), thread)])]),
                            )]),
                        );
                    }
                } else {
                    groups.insert(
                        out_type.to_owned(),
                        HashMap::from([(
                            base.clone(),
                            HashMap::from([(
                                bbv_type.clone(),
                                HashMap::from([(pid, vec![(entry.path(), thread)])]),
                            )]),
                        )]),
                    );
                }
            }
        }

        for (out_type, bases) in groups {
            for (base, bbv_types) in bases {
                for (bbv_type, pids) in &bbv_types {
                    let multiple_pids = pids.len() > 1;

                    for (pid, threads) in pids {
                        let multiple_threads = threads.len() > 1;

                        for (orig_path, thread) in threads {
                            let mut new_file_name = self.prefix();

                            if multiple_pids {
                                if let Some(pid) = pid.as_ref() {
                                    write!(new_file_name, "{pid}").unwrap();
                                }
                            }

                            if multiple_threads
                                && bbv_type.as_ref().is_some_and(|b| b.starts_with(".bb"))
                            {
                                let width = threads.len().ilog10() as usize + 1;

                                let thread = thread[1..]
                                    .parse::<usize>()
                                    .expect("The thread from the regex should be a number");

                                write!(new_file_name, ".t{thread:0width$}").unwrap();
                            }

                            if let Some(bbv_type) = &bbv_type {
                                new_file_name.push_str(bbv_type);
                            }

                            new_file_name.push_str(&out_type);

                            if let Some(base) = &base {
                                new_file_name.push_str(base);
                            }

                            let from = orig_path;
                            let to = from.with_file_name(new_file_name);

                            debug!(
                                "Sanitizing bbv file from '{}' to '{}'",
                                from.display(),
                                to.display()
                            );
                            std::fs::rename(from, to)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Sanitize file names of all tools if not sanitized by a more specific method
    ///
    /// The pids are removed from the file name if there was only a single process (pid).
    /// Additionally, we check for empty files and remove them.
    pub fn sanitize_generic(&self) -> Result<()> {
        // key: base => vec: path, pid
        type Group = HashMap<Option<String>, Vec<(PathBuf, Option<String>)>>;

        // key: .(out|log|xtree|xleak)
        let mut groups: HashMap<String, Group> = HashMap::new();
        for entry in self.walk_dir(None)? {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            let Some(haystack) = self.strip_prefix(&file_name) else {
                continue;
            };

            if let Some(caps) = GENERIC_ORIG_FILENAME_RE.captures(haystack) {
                if entry.metadata()?.size() == 0 {
                    std::fs::remove_file(entry.path())?;
                    continue;
                }

                // Don't sanitize old files.
                let base = if let Some(base) = caps.name("base") {
                    if base.as_str() == ".old" {
                        continue;
                    }

                    Some(base.as_str().to_owned())
                } else {
                    None
                };

                let out_type = caps.name("type").unwrap().as_str();
                let pid = caps.name("pid").map(|p| format!(".{}", &p.as_str()[2..]));

                if let Some(bases) = groups.get_mut(out_type) {
                    if let Some(pids) = bases.get_mut(&base) {
                        pids.push((entry.path(), pid));
                    } else {
                        bases.insert(base, vec![(entry.path(), pid)]);
                    }
                } else {
                    groups.insert(
                        out_type.to_owned(),
                        HashMap::from([(base, vec![(entry.path(), pid)])]),
                    );
                }
            }
        }

        for (out_type, bases) in groups {
            for (base, pids) in bases {
                let multiple_pids = pids.len() > 1;
                for (orig_path, pid) in pids {
                    let mut new_file_name = self.prefix();

                    if multiple_pids {
                        if let Some(pid) = pid.as_ref() {
                            write!(new_file_name, "{pid}").unwrap();
                        }
                    }

                    new_file_name.push_str(&out_type);

                    if let Some(base) = &base {
                        new_file_name.push_str(base);
                    }

                    let from = orig_path;
                    let to = from.with_file_name(new_file_name);

                    debug!("Sanitizing from '{}' to '{}'", from.display(), to.display());
                    std::fs::rename(from, to)?;
                }
            }
        }

        Ok(())
    }

    /// Sanitize file names for a specific tool
    ///
    /// Empty files are cleaned up. For more details on a specific tool see the respective
    /// `sanitize_<tool>` method.
    pub fn sanitize(&self) -> Result<()> {
        match self.tool {
            ValgrindTool::Callgrind => self.sanitize_callgrind()?,
            ValgrindTool::BBV => self.sanitize_bbv()?,
            _ => self.sanitize_generic()?,
        }

        Ok(())
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
    #[case::out(".out", vec![("type", "out")])]
    #[case::pid_out(".2049595.out", vec![("pid", "2049595"), ("type", "out")])]
    #[case::pid_thread_out(
        ".2049595.t1.out",
        vec![("pid", "2049595"), ("tid", "1"), ("type", "out")]
    )]
    #[case::pid_thread_part_out(
        ".2049595.t1.p1.out",
        vec![("pid", "2049595"), ("tid", "1"), ("part", "1"), ("type", "out")]
    )]
    #[case::out_old(".out.old", vec![("type", "out"), ("base", "old")])]
    #[case::pid_out_old(
        ".2049595.out.old",
        vec![("pid", "2049595"), ("type", "out"), ("base", "old")]
    )]
    #[case::pid_thread_out_old(
        ".2049595.t1.out.old",
        vec![("pid", "2049595"), ("tid", "1"), ("type", "out"), ("base", "old")]
    )]
    #[case::pid_thread_part_out_old(
        ".2049595.t1.p1.out.old",
        vec![
            ("pid", "2049595"),
            ("tid", "1"),
            ("part", "1"),
            ("type", "out"),
            ("base", "old")
        ]
    )]
    #[case::out_base(".out.base@name", vec![("type", "out"), ("base", "base@name")])]
    #[case::pid_out_base(
        ".2049595.out.base@name",
        vec![("pid", "2049595"), ("type", "out"), ("base", "base@name")]
    )]
    #[case::pid_thread_out_base(
        ".2049595.t1.out.base@name",
        vec![("pid", "2049595"), ("tid", "1"), ("type", "out"), ("base", "base@name")]
    )]
    #[case::pid_thread_part_out_base(
        ".2049595.t1.p1.out.base@name",
        vec![
            ("pid", "2049595"),
            ("tid", "1"),
            ("part", "1"),
            ("type", "out"),
            ("base", "base@name")
        ]
    )]
    #[case::bb_out(".bb.out", vec![("bbv", "bb"), ("type", "out")])]
    #[case::pc_out(".pc.out", vec![("bbv", "pc"), ("type", "out")])]
    #[case::pid_bb_out(".123.bb.out", vec![("pid", "123"), ("bbv", "bb"), ("type", "out")])]
    #[case::pid_thread_bb_out(
        ".123.t1.bb.out",
        vec![("pid", "123"), ("tid", "1"), ("bbv", "bb"), ("type", "out")]
    )]
    #[case::log(".log", vec![("type", "log")])]
    #[case::xtree(".xtree", vec![("type", "xtree")])]
    #[case::xtree_old(".xtree.old", vec![("type", "xtree"), ("base", "old")])]
    #[case::xleak(".xleak", vec![("type", "xleak")])]
    #[case::xleak_old(".xleak.old", vec![("type", "xleak"), ("base", "old")])]
    fn test_real_file_name_regex(#[case] haystack: &str, #[case] expected: Vec<(&str, &str)>) {
        assert!(REAL_FILENAME_RE.is_match(haystack));

        let caps = REAL_FILENAME_RE.captures(haystack).unwrap();
        for (name, value) in expected {
            assert_eq!(caps.name(name).unwrap().as_str(), value);
        }
    }

    #[rstest]
    #[case::out(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.out",
        "callgrind.bench_thread_in_subprocess.two.log"
    )]
    #[case::out_old(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.out.old",
        "callgrind.bench_thread_in_subprocess.two.log.old"
    )]
    #[case::pid_out(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.out",
        "callgrind.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::pid_tid_out(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.t1.out",
        "callgrind.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::pid_tid_part_out(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.t1.p2.out",
        "callgrind.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::pid_out_old(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.out.old",
        "callgrind.bench_thread_in_subprocess.two.123.log.old"
    )]
    #[case::pid_tid_part_out_old(
        ValgrindTool::Callgrind,
        "callgrind.bench_thread_in_subprocess.two.123.t1.p2.out.old",
        "callgrind.bench_thread_in_subprocess.two.123.log.old"
    )]
    #[case::bb_out(
        ValgrindTool::BBV,
        "exp-bbv.bench_thread_in_subprocess.two.bb.out",
        "exp-bbv.bench_thread_in_subprocess.two.log"
    )]
    #[case::bb_pid_out(
        ValgrindTool::BBV,
        "exp-bbv.bench_thread_in_subprocess.two.123.bb.out",
        "exp-bbv.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::bb_pid_tid_out(
        ValgrindTool::BBV,
        "exp-bbv.bench_thread_in_subprocess.two.123.t1.bb.out",
        "exp-bbv.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::xtree(
        ValgrindTool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xtree",
        "memcheck.bench_thread_in_subprocess.two.log"
    )]
    #[case::xtree_old(
        ValgrindTool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xtree.old",
        "memcheck.bench_thread_in_subprocess.two.log.old"
    )]
    #[case::xtree_pid(
        ValgrindTool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.123.xtree",
        "memcheck.bench_thread_in_subprocess.two.123.log"
    )]
    #[case::xleak(
        ValgrindTool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xleak",
        "memcheck.bench_thread_in_subprocess.two.log"
    )]
    #[case::xleak_old(
        ValgrindTool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.xleak.old",
        "memcheck.bench_thread_in_subprocess.two.log.old"
    )]
    #[case::xleak_pid(
        ValgrindTool::Memcheck,
        "memcheck.bench_thread_in_subprocess.two.123.xleak",
        "memcheck.bench_thread_in_subprocess.two.123.log"
    )]
    fn test_tool_output_path_log_path_of(
        #[case] tool: ValgrindTool,
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
            ValgrindTool::Callgrind,
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
            ValgrindTool::Callgrind,
            &BaselineKind::Old,
            &PathBuf::from("/root"),
            &ModulePath::new("hello::world"),
            "bench_thread_in_subprocess.two",
            false,
        )
        .unwrap();

        assert!(output_path
            .log_path_of(&PathBuf::from(
                "/root/not/here/bench_thread_in_subprocess.two/callgrind.\
                 bench_thread_in_subprocess.two.out"
            ))
            .is_none());
    }
}
