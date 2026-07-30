//! Module containing the basic cachegrind parser elements
use anyhow::{Result, anyhow};
use log::trace;

use super::model::Metrics;

/// The properties and header data of a cachegrind output file
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CachegrindProperties {
    /// The executed command with command-line arguments
    pub cmd: String,
    /// The `desc:` fields
    pub desc: Vec<String>,
    /// The prototype for all metrics in this file
    pub metrics_prototype: Metrics,
}

/// Parse the output file header of a cachegrind out file
///
/// The format as described here
/// <https://valgrind.org/docs/manual/cg-manual.html#cg-manual.impl-details.file-format>:
///
/// ```text
/// file         ::= desc_line* cmd_line events_line data_line+ summary_line
/// desc_line    ::= "desc:" ws? non_nl_string
/// cmd_line     ::= "cmd:" ws? cmd
/// events_line  ::= "events:" ws? (event ws)+
/// data_line    ::= file_line | fn_line | count_line
/// file_line    ::= "fl=" filename
/// fn_line      ::= "fn=" fn_name
/// count_line   ::= line_num (ws+ count)* ws*
/// summary_line ::= "summary:" ws? count (ws+ count)+ ws*
/// count        ::= num
/// ```
/// Parse the callgrind output files header
pub fn parse_header<I>(iter: &mut I) -> Result<CachegrindProperties>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    let mut metrics_prototype: Option<Metrics> = None;
    let mut desc: Vec<String> = vec![];
    let mut cmd: Option<String> = None;

    for line in iter {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match line.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
            Some(("desc", value)) if !value.starts_with("Option:") => {
                trace!("Using description '{value}' from line: '{line}'");
                desc.push(value.to_owned());
            }
            Some(("cmd", value)) => {
                trace!("Using cmd '{value}' from line: '{line}'");
                cmd = Some(value.to_owned());
            }
            // The events line is the last line in the header which is mandatory. The summary line
            // is usually the last line, but it is only optional. So, we break out of
            // the loop here and stop the parsing.
            Some(("events", events)) => {
                trace!("Using events '{events}' from line: '{line}'");
                metrics_prototype = Some(
                    events
                        .split_ascii_whitespace()
                        .map(str::parse)
                        .collect::<Result<Metrics>>()?,
                );
                break;
            }
            // None is actually a malformed header line we just ignore here
            // Some(_) includes `^event:` lines
            None | Some(_) => {}
        }
    }

    Ok(CachegrindProperties {
        metrics_prototype: metrics_prototype
            .ok_or_else(|| anyhow!("Header field 'events' must be present"))?,
        desc,
        cmd: cmd.ok_or_else(|| anyhow!("Header field 'cmd' must be present"))?,
    })
}
