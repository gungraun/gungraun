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
| >=0.16.0                       | [summary.v6.schema.json] |

Each line of json output (if not `pretty-json`) is a summary of a single
benchmark, and you may want to combine all benchmarks in an array. You can do so
for example with `jq`

`cargo bench -- --output-format=json | jq -s`

which transforms `{...}\n{...}` into `[{...},{...}]`.

Instead of, or in addition to changing the terminal output, it's possible to
save a summary file for each benchmark with `--save-summary=json|pretty-json`
(env: `GUNGRAUN_SAVE_SUMMARY`). The `summary.json` files are stored next to the
usual benchmark output files in the `target/gungraun` directory.

<!-- TODO: Add gungraun-summary description -->

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
