# Machine-Readable Output

With `--output-format=default|json|pretty-json` (env: `GUNGRAUN_OUTPUT_FORMAT`)
you can change the terminal output format to the machine-readable json format.
The json schemas fully describing the json output are stored here:

| Iai-Callgrind/Gungraun version | Schema version           |
| ------------------------------ | ------------------------ |
| >=0.9.0,<0.11.0                | [summary.v1.schema.json] |
| >=0.11.0,<0.14.0               | [summary.v2.schema.json] |
| >=0.14.0,<0.15.0               | [summary.v3.schema.json] |
| >=0.15.0,<0.15.2               | [summary.v4.schema.json] |
| >=0.15.2,<0.16.0               | [summary.v5.schema.json] |
| >=0.16.0,<0.20.0               | [summary.v6.schema.json] |
| >=0.20.0                       | [summary.v7.schema.json] |

Each line of json output (if not `pretty-json`) is a summary of a single
benchmark, and you may want to combine all benchmarks in an array. You can do so
for example with `jq`

`cargo bench -- --output-format=json | jq -s`

which transforms `{...}\n{...}` into `[{...},{...}]`.

Instead of, or in addition to changing the terminal output, it's possible to
save a summary file for each benchmark with `--save-summary=json|pretty-json`
(env: `GUNGRAUN_SAVE_SUMMARY`).

## Parsing summary.json

Each `summary.json` contains the result of one benchmark. The file is stored in
the benchmark's [output directory](out_directory.md), so consumers can process
individual benchmarks without first splitting the terminal's JSON stream.

The examples in this section use schema version 7. In this format, tool data is
stored in `profiles`, with aggregate metrics under paths such as
`.data.total.metrics.Ir`. Metric values are plain JSON numbers in `values.new`
and, when a baseline exists, `values.old`. A comparison is stored in `change`.
Its `diff_pct` and `factor` values are strings because they can contain values
such as `"inf"`.

Optional fields are omitted when they have no value. The exception is
`baselines`, a fixed two-element array whose missing entries are `null`.

<details>
<summary>Complete schema v7 summary.json example</summary>

```json
{{#include summary.example.json}}
```

</details>

### gungraun-summary

The [`gungraun-summary`] crate provides typed Rust data structures for reading
summary files. Its major version tracks the latest supported schema version, so
use version 7 when consuming schema v7 files:

```toml
[dependencies]
gungraun-summary = "7"
```

The version modules are self-contained. If the version of a file is already
known, parse it directly with the `v7` module:

```rust,no_run
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = gungraun_summary::v7::parse(Path::new("summary.json"))?;
    println!("{}", summary.function_name);
    Ok(())
}
```

For input whose version is not known in advance, `util::parse` first reads the
top-level `version` field and then selects the matching representation:

```rust,no_run
use std::path::Path;

use gungraun_summary::util::{SummaryByVersion, parse};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = match parse(Path::new("summary.json"))? {
        SummaryByVersion::V7(summary) => summary,
        _ => return Err("expected a schema v7 summary".into()),
    };
    println!("{}", summary.function_name);
    Ok(())
}
```

### Examples with jq

The following commands read the complete [example summary](summary.example.json)
shown above. Set a variable to the path of any schema v7 `summary.json` to use
them with your own results:

```shell
summary=summary.example.json
```

Extract the new Callgrind instruction-read count:

```shell
jq '.profiles[] | select(.tool == "Callgrind") | .data.total.metrics.Ir.values.new' "$summary"
```

```text
687
```

Convert the Callgrind metrics map to a list containing each metric's values and
percentage change. The final slice keeps this example's output short; remove
`| .[:2]` to return all metrics.

```shell
jq '[
  .profiles[]
  | select(.tool == "Callgrind")
  | .data.total.metrics
  | to_entries[]
  | {
      metric: .key,
      new: .value.values.new,
      old: .value.values.old,
      diff_pct: (.value.change.diff_pct? | tonumber)
    }
] | .[:2]' "$summary"
```

```json
[
    {
        "metric": "D1MissRate",
        "new": 0.33444816053511706,
        "old": 0.33444816053511706,
        "diff_pct": 0
    },
    {
        "metric": "D1mr",
        "new": 1,
        "old": 1,
        "diff_pct": 0
    }
]
```

Filter metrics by the absolute percentage change. This example uses a threshold
of zero so it produces output for the unchanged sample; use a value such as `5`
to find changes of at least five percent. The explicit infinity check is needed
before `tonumber` because `diff_pct` can be `"inf"` or `"-inf"`.

```shell
jq --argjson threshold 0 '
  first(
    .profiles[]
    | select(.tool == "Callgrind")
    | .data.total.metrics
    | to_entries[]
    | select(
        .value.change.diff_pct? as $diff
        | $diff != "inf"
          and $diff != "-inf"
          and (($diff | tonumber | fabs) >= $threshold)
      )
    | {
        metric: .key,
        new: .value.values.new,
        old: .value.values.old,
        diff_pct: .value.change.diff_pct
      }
  )
' "$summary"
```

```json
{
    "metric": "D1MissRate",
    "new": 0.33444816053511706,
    "old": 0.33444816053511706,
    "diff_pct": "0"
}
```

Extract the fields that identify a benchmark:

```shell
jq '{ group, function_name, module_path, id }' "$summary"
```

```json
{
    "group": "fibonacci",
    "function_name": "bench_fibonacci_with_config",
    "module_path": "test_lib_bench_intro::fibonacci::bench_fibonacci_with_config",
    "id": null
}
```

[summary.v1.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v1.schema.json
[summary.v2.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v2.schema.json
[summary.v3.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v3.schema.json
[summary.v4.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v4.schema.json
[summary.v5.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v5.schema.json
[summary.v6.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v6.schema.json
[summary.v7.schema.json]:
    https://github.com/gungraun/gungraun/blob/main/crates/gungraun-summary/schemas/summary.v7.schema.json
[`gungraun-summary`]: https://docs.rs/gungraun-summary/latest/gungraun_summary/
