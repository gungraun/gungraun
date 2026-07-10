//! Module containing dhat files for json output parser the

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use super::model::DhatData;
use super::tree::{RootTree, Tree};
use crate::api::{EntryPoint, SanitizeOutput};
use crate::runner::tool::logfile_parser;
use crate::runner::tool::parser::{Header, Parser, ParserOutput};
use crate::runner::tool::path::ToolOutputPath;

/// The dhat output file json parser
#[derive(Debug)]
pub struct JsonParser {
    entry_point: EntryPoint,
    frames: Vec<String>,
    output_path: ToolOutputPath,
    sanitize_output: SanitizeOutput,
}

impl JsonParser {
    /// Creates a new `JsonParser`.
    pub fn new(
        output_path: ToolOutputPath,
        entry_point: EntryPoint,
        frames: Vec<String>,
        sanitize_output: SanitizeOutput,
    ) -> Self {
        Self {
            entry_point,
            frames,
            output_path,
            sanitize_output,
        }
    }
}

impl Parser for JsonParser {
    fn parse_single(&self, path: PathBuf) -> Result<ParserOutput> {
        let mut dhat_data = parse(&path)
            .with_context(|| format!("Error opening dhat output file '{}'", path.display()))?;

        let parent_pid = if let Some(logfile) = self.output_path.log_path_of(&path) {
            let file = File::open(&logfile)
                .with_context(|| format!("Error opening dhat log file '{}'", logfile.display()))?;

            let iter = BufReader::new(file).lines();
            let header = logfile_parser::parse_header(&logfile, iter)?;

            assert_eq!(
                header.pid, dhat_data.metadata.pid,
                "The pid of the json and log file should be equal"
            );

            header.parent_pid
        } else {
            None
        };

        let header = Header {
            command: dhat_data.metadata.command.clone(),
            pid: dhat_data.metadata.pid,
            parent_pid,
            thread: None,
            part: None,
            desc: vec![],
        };

        dhat_data.filter_program_points(&self.entry_point, &self.frames);

        let metrics = if matches!(
            self.sanitize_output,
            SanitizeOutput::KeepOrig | SanitizeOutput::Yes
        ) {
            // Instead of using a DhatTree, construction and then deconstruction a whole tree, it is
            // more efficient to use the RootTree with the filtered original program points
            // directly. However, the dhat data reconstructed from the root tree needs to be
            // sanitized because the frame table might have changed.
            let program_points = dhat_data.program_points.clone();
            let tree = RootTree::from_json(dhat_data);
            let metrics = tree.metrics();

            if self.sanitize_output == SanitizeOutput::KeepOrig {
                let orig = path.with_extension("out.orig");
                std::fs::copy(&path, orig).with_context(|| {
                    format!(
                        "Backing up original dhat data '{}' should succeed",
                        path.display()
                    )
                })?;
            }

            let mapping_table = tree.mapping_table();
            let mut new_data = DhatData {
                program_points,
                ..tree.into()
            };

            new_data.sanitize(&mapping_table);

            serde_json::to_writer(File::create(&path)?, &new_data).with_context(|| {
                format!(
                    "Failed serializing sanitized dhat output to '{}'",
                    path.display()
                )
            })?;

            metrics
        } else {
            RootTree::from_json(dhat_data).metrics()
        };

        Ok(ParserOutput {
            path,
            header,
            details: vec![],
            metrics,
        })
    }

    fn get_output_path(&self) -> &ToolOutputPath {
        &self.output_path
    }
}

/// Parse the dhat output file at `path` into [`DhatData`]
pub fn parse(path: &Path) -> Result<DhatData> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(|error| {
        anyhow!(
            "Error parsing dhat output file '{}': {error}",
            path.display()
        )
    })
}
