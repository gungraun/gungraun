// spell-checker: ignore redir
use std::borrow::Cow;
use std::fmt::Display;
use std::io::{BufRead, BufReader, Write, stderr};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::LazyLock;

use client_request_tests::MARKER;
use regex::Regex;

static STRIP_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(==|--|\*\*)([0-9:.]+\s+)?[0-9]+(==|--|\*\*)\s*(?<rest>.*)$")
        .expect("Regex should compile")
});
static CALLGRIND_EXCLUDED_LINES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(For interactive control,)").expect("Regex should compile"));
static CALLGRIND_RM_DUMP_TO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(Dump to).*$").expect("Regex should compile"));
static CALLGRIND_RM_BB_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(at BB\s+)([0-9]+)(\s+.*)$").expect("Regex should compile"));
static CALLGRIND_RM_ADDR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((at|by)\s+)(0x[0-9A-Za-z]+\s*:.*)$").expect("Regex should compile")
});
static CALLGRIND_RM_NUM_REFS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((I|D|LL)\s*refs:)[ 0-9,()+rdw]*$").expect("Regex should compile")
});
static CALLGRIND_RM_NUM_MISS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((I1|D1|LL|LLi|LLd)\s*(misses|miss rate):)[ 0-9,()+rdw%.]*$")
        .expect("Regex should compile")
});
static CALLGRIND_RM_NUM_RATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((Branches|Mispredicts|Mispred rate):)[ 0-9,()+condi%.]*$")
        .expect("Regex should compile")
});
static CALLGRIND_RM_NUM_COLLECTED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(Collected\s*:)[ 0-9]*$").expect("Regex should compile"));
static CALLGRIND_RM_LINE_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\(.*:)([0-9]+)(\))\s*$").expect("Regex should compile"));
static BACKTRACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^((at|by)\s*0x[0-9A-Za-z]+\s*:)").expect("Regex should compile"));
static MEMCHECK_CHECKED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*Checked\s*)([0-9,.]+)(\s*bytes)\s*$").expect("Regex should compile")
});
static MEMCHECK_TOTAL_HEAP_USAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?i:(\s*total heap usage:\s*))(.*)$").expect("Regex should compile")
});
static MEMCHECK_LEAK_SUMMARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i:(\s*(definitely lost|indirectly lost|possibly lost|",
        r"still reachable|suppressed):\s*))([ 0-9,()+.]*)(\s*bytes in\s*)",
        r"([ 0-9,()+.]*)(\s*blocks\s*)$"
    ))
    .expect("Regex should compile")
});
static MEMCHECK_RM_NUMBERS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[+-]?[0-9][0-9,.]*").expect("Regex should compile"));
static MEMORY_ADDRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"0x[0-9A-Za-z]+").expect("Regex should compile"));
static CACHEGRIND_NUM_REFS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((I|D|LL)\s*refs:\s*)([ 0-9,()+rdw]*)\s*$").expect("Regex should compile")
});
static WARNING_EXIDX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[ ]*Warning: whilst reading EXIDX:.*$").expect("Regex should compile")
});
static READING_EXIDX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ ]*Reading EXIDX entries:.*$").expect("Regex should compile"));
static NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9]+").expect("Regex should compile"));
static REDIR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(REDIR:)(.*)").expect("Regex should compile"));

#[derive(Debug)]
enum Tool {
    Callgrind,
    Memcheck,
    Helgrind,
    Cachegrind,
}

impl FromStr for Tool {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "callgrind" => Ok(Tool::Callgrind),
            "memcheck" => Ok(Tool::Memcheck),
            "helgrind" => Ok(Tool::Helgrind),
            "cachegrind" => Ok(Tool::Cachegrind),
            tool => Err(format!("Unsupported tool: {tool}")),
        }
    }
}

impl Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{self:?}").to_ascii_lowercase().as_str())
    }
}

fn callgrind_filter(path: &Path, bytes: &[u8], writer: &mut impl Write) {
    let path_re =
        Regex::new(format!(r"({})", path.display()).as_str()).expect("Regex should compile");

    #[derive(Debug, PartialEq, Eq)]
    enum State {
        Header,
        Body,
    }
    let mut state = State::Header;
    let mut is_backtrace = false;
    for line in BufReader::new(bytes).lines().map(Result::unwrap) {
        if state == State::Header {
            if line.contains(MARKER) {
                state = State::Body;
            }
            continue;
        }

        let rest = STRIP_PREFIX_RE
            .captures(&line)
            .unwrap_or_else(|| {
                panic!("Callgrind output line should be a valid output line: was {line}")
            })
            .name("rest")
            .unwrap()
            .as_str();

        if rest.starts_with("Symbol match:") {
            continue;
        };

        // backtraces are too different on different targets
        if BACKTRACE_RE.is_match(rest) {
            if !is_backtrace {
                writeln!(writer, "<__BACKTRACE__>").unwrap();
                is_backtrace = true;
            }
            continue;
        } else {
            is_backtrace = false;
        }

        let replaced = path_re.replace_all(rest, "<__FILTER__>");
        let replaced = CALLGRIND_RM_ADDR_RE.replace_all(&replaced, "$1<__FILTER__>");
        let replaced = CALLGRIND_RM_BB_NUM_RE.replace_all(&replaced, "$1<__FILTER__>$3");
        let replaced = CALLGRIND_RM_LINE_NUM_RE.replace_all(&replaced, "$1<__FILTER__>$3");
        let rest = replaced.deref();
        if !CALLGRIND_EXCLUDED_LINES_RE.is_match(rest) {
            if let Some(caps) = CALLGRIND_RM_NUM_REFS_RE.captures(rest) {
                writeln!(writer, "{}", caps.get(1).unwrap().as_str()).unwrap();
            } else if let Some(caps) = CALLGRIND_RM_NUM_MISS_RE.captures(rest) {
                writeln!(writer, "{}", caps.get(1).unwrap().as_str()).unwrap();
            } else if let Some(caps) = CALLGRIND_RM_NUM_RATE_RE.captures(rest) {
                writeln!(writer, "{}", caps.get(1).unwrap().as_str()).unwrap();
            } else if let Some(caps) = CALLGRIND_RM_NUM_COLLECTED_RE.captures(rest) {
                writeln!(writer, "{}", caps.get(1).unwrap().as_str()).unwrap();
            } else if let Some(caps) = CALLGRIND_RM_DUMP_TO_RE.captures(rest) {
                writeln!(writer, "{}", caps.get(1).unwrap().as_str()).unwrap();
            } else {
                writeln!(writer, "{rest}").unwrap();
            }
        }
    }
}

fn memcheck_filter(bytes: &[u8], writer: &mut impl Write) {
    #[derive(Debug, PartialEq, Eq)]
    enum State {
        Header,
        Body,
    }
    let mut state = State::Header;
    let mut is_backtrace = false;
    for line in BufReader::new(bytes).lines().map(Result::unwrap) {
        if state == State::Header {
            if line.contains(MARKER) {
                state = State::Body;
            }
            continue;
        }
        let rest = STRIP_PREFIX_RE
            .captures(&line)
            .unwrap_or_else(|| {
                panic!("Memcheck output line should be a valid output line: was {line}")
            })
            .name("rest")
            .unwrap()
            .as_str();

        // backtraces are too different on different targets
        if BACKTRACE_RE.is_match(rest) {
            if !is_backtrace {
                writeln!(writer, "<__BACKTRACE__>").unwrap();
                is_backtrace = true;
            }
            continue;
        } else {
            is_backtrace = false;
        }

        let replaced = REDIR_RE.replace_all(rest, "$1 <__FILTER__>");
        let replaced = MEMCHECK_CHECKED_RE.replace_all(&replaced, "$1<__FILTER__>$3");
        let replaced = MEMCHECK_TOTAL_HEAP_USAGE_RE.replace_all(&replaced, "$1<__FILTER__>");
        let replaced =
            MEMCHECK_LEAK_SUMMARY_RE.replace_all(&replaced, "$1<__FILTER__> $4<__FILTER__> $6");
        let replaced = MEMORY_ADDRESS_RE.replace_all(&replaced, "<__FILTER__>");
        let replaced = MEMCHECK_RM_NUMBERS_RE.replace_all(&replaced, "<__NUMBER__>");
        writeln!(writer, "{replaced}").unwrap();
    }
}

fn cachegrind_filter(bytes: &[u8], writer: &mut impl Write) {
    #[derive(Debug, PartialEq, Eq)]
    enum State {
        Header,
        Body,
    }
    let mut state = State::Header;
    for line in BufReader::new(bytes).lines().map(Result::unwrap) {
        if state == State::Header {
            if line.contains(MARKER) {
                state = State::Body;
            }
            continue;
        }

        let rest = STRIP_PREFIX_RE
            .captures(&line)
            .unwrap_or_else(|| {
                panic!("Cachegrind output line should be a valid output line: was {line}")
            })
            .name("rest")
            .unwrap()
            .as_str();

        if WARNING_EXIDX_RE.is_match(rest) {
            continue;
        }
        let rest = if READING_EXIDX_RE.is_match(rest) {
            NUMBER_RE.replace_all(rest, "<__NUMBER__>")
        } else {
            Cow::Borrowed(rest)
        };

        let replaced = CACHEGRIND_NUM_REFS_RE.replace_all(&rest, "$1<__FILTER__>");
        writeln!(writer, "{replaced}").unwrap();
    }
}

pub fn get_valgrind_command() -> Command {
    let valgrind = if let Ok(dir) = std::env::var("VALGRIND_REQUESTS_VALGRIND_PATH") {
        PathBuf::from(dir).join("valgrind")
    } else {
        // This works inside the qemu image during a cross run, too
        PathBuf::from("valgrind")
    };

    Command::new(valgrind)
}

fn main() {
    let mut env_args = std::env::args();
    let base_dir = {
        let exe = PathBuf::from(env_args.next().unwrap());
        exe.parent().unwrap().to_owned()
    };

    let expected_exit_code = env_args.next().unwrap().parse::<i32>().unwrap();

    let tool = {
        let tool = env_args.next().unwrap();
        Tool::from_str(tool.strip_prefix("--tool=").unwrap())
            .expect("Tool should be a valid valgrind tool")
    };

    let valgrind_args = {
        let args = env_args.next().unwrap();
        shlex::split(args.strip_prefix("--valgrind-args=").unwrap()).unwrap()
    };

    let bin = {
        let bin = env_args.next().unwrap();
        base_dir.join(bin.strip_prefix("--bin=").unwrap())
    };

    let bin_args = {
        if let Some(args) = env_args.next() {
            shlex::split(args.strip_prefix("--bin-args=").unwrap()).unwrap()
        } else {
            vec![]
        }
    };

    let mut cmd = get_valgrind_command();
    cmd.arg(format!("--tool={tool}"))
        .args(valgrind_args)
        .arg(&bin)
        .args(bin_args);
    let output = cmd.output().unwrap();

    if let Some(code) = output.status.code() {
        if code == expected_exit_code {
            match tool {
                Tool::Callgrind => callgrind_filter(&bin, &output.stderr, &mut stderr()),
                Tool::Cachegrind => cachegrind_filter(&output.stderr, &mut stderr()),
                Tool::Memcheck => memcheck_filter(&output.stderr, &mut stderr()),
                _ => {
                    stderr().write_all(&output.stderr).unwrap();
                }
            }
        } else {
            let stderr = stderr();
            let mut stderr = stderr.lock();
            eprintln!("Unexpected exit code '{code}' when running {cmd:?}");
            eprintln!("STDOUT:");
            stderr.write_all(&output.stdout).unwrap();
            eprintln!("STDERR:");
            stderr.write_all(&output.stderr).unwrap();
        }

        std::process::exit(code);
    } else {
        eprintln!("{output:?}");
        std::process::exit(-1);
    }
}
