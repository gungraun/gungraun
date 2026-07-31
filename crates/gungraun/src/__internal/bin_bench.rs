use std::collections::HashSet;

use crate::__internal::error::{Error, Errors};
use crate::__internal::{
    InternalBinaryBenchmark, InternalBinaryBenchmarkBench, InternalBinaryBenchmarkConfig,
    InternalBinaryBenchmarkGroup, InternalBinaryBenchmarkGroups, InternalCommandKind,
    InternalMacroBinBench, ModulePath,
};
use crate::{BenchmarkId, Tool};

const UNKNOWN_ARGS: &str = "N/A";

pub type InternalMacroBinBenches = &'static [&'static (
    &'static str,
    fn() -> Option<InternalBinaryBenchmarkConfig>,
    &'static [InternalMacroBinBench],
)];

#[derive(Debug)]
pub struct GroupsBuilder {
    bench_ids: HashSet<BenchmarkId>,
    binary_benchmark_ids: HashSet<BenchmarkId>,
    errors: Errors,
    groups: InternalBinaryBenchmarkGroups,
}

impl GroupsBuilder {
    pub fn new(
        config: Option<InternalBinaryBenchmarkConfig>,
        args: Vec<String>,
        has_setup: bool,
        has_teardown: bool,
    ) -> Self {
        let groups = if cfg!(feature = "cachegrind") {
            InternalBinaryBenchmarkGroups {
                config: config.unwrap_or_default(),
                groups: Vec::default(),
                command_line_args: args,
                has_setup,
                has_teardown,
                default_tool: Tool::Cachegrind,
            }
        } else {
            InternalBinaryBenchmarkGroups {
                config: config.unwrap_or_default(),
                groups: Vec::default(),
                command_line_args: args,
                has_setup,
                has_teardown,
                default_tool: Tool::Callgrind,
            }
        };

        Self {
            groups,
            errors: Errors::default(),
            bench_ids: HashSet::default(),
            binary_benchmark_ids: HashSet::default(),
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub fn add_group(
        &mut self,
        group: crate::BinaryBenchmarkGroup,
        id: String,
        module_path: &str,
        is_attribute: bool,
        config: Option<InternalBinaryBenchmarkConfig>,
        has_setup: bool,
        has_teardown: bool,
        compare_by_id: Option<bool>,
        max_parallel: Option<usize>,
        benches: InternalMacroBinBenches,
    ) {
        let mut internal_group = InternalBinaryBenchmarkGroup {
            id,
            config,
            has_setup,
            has_teardown,
            compare_by_id,
            max_parallel,
            ..Default::default()
        };

        if is_attribute {
            Self::high_level(&mut internal_group, benches);
        } else {
            self.low_level(&mut internal_group, group, module_path);
        }

        self.groups.groups.push(internal_group);
    }

    fn add_binary_benchmark_id(&mut self, id: &BenchmarkId, module_path: &ModulePath) -> bool {
        self.bench_ids.clear();

        if let Err(message) = id.validate() {
            self.errors.add(Error::new(module_path, &message));
            return false;
        }

        if !self.binary_benchmark_ids.insert(id.clone()) {
            self.errors
                .add(Error::new(module_path, "Duplicate binary benchmark id"));
            return false;
        }
        true
    }

    fn add_command_id(
        &mut self,
        id: &BenchmarkId,
        command: &InternalCommandKind,
        module_path: &ModulePath,
    ) -> bool {
        if let Err(message) = id.validate() {
            self.errors.add(Error::new(module_path, &message));
            return false;
        }

        match command {
            crate::__internal::InternalCommandKind::Default(_) => {
                if !self.bench_ids.insert(id.clone()) {
                    self.errors
                        .add(Error::new(module_path, &format!("Duplicate id: '{id}'")));
                    return false;
                }

                true
            }
            crate::__internal::InternalCommandKind::Iter(commands) => {
                for index in 0..commands.len() {
                    let bench_id: BenchmarkId = format!("{id}_{index}").into();
                    if !self.bench_ids.insert(bench_id.clone()) {
                        self.errors.add(Error::new(
                            module_path,
                            &format!("Duplicate id: '{bench_id}'"),
                        ));
                        return false;
                    }
                }

                true
            }
        }
    }

    /// Adds a high-level api benchmark to the `group` parsing the `benches`.
    fn high_level(group: &mut InternalBinaryBenchmarkGroup, benches: InternalMacroBinBenches) {
        for (function_name, get_config, macro_bin_benches) in benches {
            let mut internal_binary_benchmark = InternalBinaryBenchmark {
                benches: vec![],
                config: get_config(),
            };
            for macro_bin_bench in *macro_bin_benches {
                let bench = InternalBinaryBenchmarkBench {
                    id: macro_bin_bench.id_display.map(str::to_owned),
                    args: macro_bin_bench.args_display.map(str::to_owned),
                    consts_display: macro_bin_bench.consts_display.map(str::to_owned),
                    function_name: (*function_name).to_owned(),
                    command: match macro_bin_bench.func {
                        crate::__internal::InternalBinFunctionKind::Iter(func) => {
                            crate::__internal::InternalCommandKind::Iter(
                                func().iter().map(Into::into).collect(),
                            )
                        }
                        crate::__internal::InternalBinFunctionKind::Default(func) => {
                            crate::__internal::InternalCommandKind::Default(Box::new(func().into()))
                        }
                    },
                    config: macro_bin_bench.config.map(|f| f()),
                    has_setup: macro_bin_bench.setup.is_some(),
                    has_teardown: macro_bin_bench.teardown.is_some(),
                };
                internal_binary_benchmark.benches.push(bench);
            }
            group.binary_benchmarks.push(internal_binary_benchmark);
        }
    }

    /// Adds a low-level api benchmark to the `internal_group` parsing the `group`.
    ///
    /// In contrast to the high-level api, we need to check for duplicate ids, missing commands ...
    /// The errors are collected and then printed instead of returning on first error and printing
    /// errors one by one.
    fn low_level(
        &mut self,
        internal_group: &mut InternalBinaryBenchmarkGroup,
        group: crate::BinaryBenchmarkGroup,
        module_path: &str,
    ) {
        let module_path = ModulePath::new(module_path).join(&internal_group.id);

        if group.binary_benchmarks.is_empty() {
            self.errors.add(Error::new(
                &module_path,
                "This group needs at least one benchmark",
            ));
            return;
        }

        self.binary_benchmark_ids.clear();
        for binary_benchmark in group.binary_benchmarks {
            let module_path = module_path.join(&binary_benchmark.id.to_string());

            if !self.add_binary_benchmark_id(&binary_benchmark.id, &module_path) {
                continue;
            }

            let mut internal_binary_benchmark = InternalBinaryBenchmark {
                benches: vec![],
                config: binary_benchmark.config,
            };

            if binary_benchmark.benches.is_empty() {
                self.errors.add(Error::new(
                    &module_path,
                    "This binary benchmark needs at least one bench",
                ));
            }

            for bench in binary_benchmark.benches {
                let module_path = module_path.join(&bench.id.to_string());

                match bench.commands.as_slice() {
                    [] => {
                        self.errors.add(Error::new(&module_path, "Missing command"));
                    }
                    [command] => {
                        if !self.add_command_id(&bench.id, command, &module_path) {
                            continue;
                        }

                        let internal_bench = InternalBinaryBenchmarkBench {
                            id: Some(bench.id.into()),
                            args: None,
                            consts_display: None,
                            function_name: binary_benchmark.id.clone().into(),
                            command: command.clone(),
                            config: bench.config.clone(),
                            has_setup: bench.setup.is_some() || binary_benchmark.setup.is_some(),
                            has_teardown: bench.teardown.is_some()
                                || binary_benchmark.teardown.is_some(),
                        };
                        internal_binary_benchmark.benches.push(internal_bench);
                    }
                    commands => {
                        for (index, command) in commands.iter().enumerate() {
                            let indexed_bench_id: BenchmarkId =
                                format!("{}_{}", bench.id, index).into();

                            if !self.add_command_id(&indexed_bench_id, command, &module_path) {
                                continue;
                            }
                            let internal_bench = InternalBinaryBenchmarkBench {
                                id: Some(indexed_bench_id.into()),
                                args: match command {
                                    InternalCommandKind::Default(_) => {
                                        Some(UNKNOWN_ARGS.to_owned())
                                    }
                                    InternalCommandKind::Iter(_) => {
                                        Some(format!("nth of {UNKNOWN_ARGS}"))
                                    }
                                },
                                consts_display: None,
                                function_name: binary_benchmark.id.to_string(),
                                command: command.clone(),
                                config: bench.config.clone(),
                                has_setup: bench.setup.is_some()
                                    || binary_benchmark.setup.is_some(),
                                has_teardown: bench.teardown.is_some()
                                    || binary_benchmark.teardown.is_some(),
                            };
                            internal_binary_benchmark.benches.push(internal_bench);
                        }
                    }
                }
            }

            internal_group
                .binary_benchmarks
                .push(internal_binary_benchmark);
        }
    }

    pub fn build(self) -> Result<InternalBinaryBenchmarkGroups, Errors> {
        if self.errors.is_empty() {
            Ok(self.groups)
        } else {
            Err(self.errors)
        }
    }
}
