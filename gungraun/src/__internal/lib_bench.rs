use crate::Tool;

type MacroLibBenches<'a> = &'a [&'a (
    &'static str,
    fn() -> Option<crate::__internal::InternalLibraryBenchmarkConfig>,
    &'a [crate::__internal::InternalMacroLibBench],
)];

#[derive(Debug)]
pub struct GroupsBuilder(crate::__internal::InternalLibraryBenchmarkGroups);

impl GroupsBuilder {
    #[cfg(feature = "cachegrind")]
    pub fn new(
        config: Option<crate::__internal::InternalLibraryBenchmarkConfig>,
        args: Vec<String>,
        has_setup: bool,
        has_teardown: bool,
    ) -> Self {
        Self(crate::__internal::InternalLibraryBenchmarkGroups {
            config: config.unwrap_or_default(),
            groups: Vec::default(),
            command_line_args: args,
            has_setup,
            has_teardown,
            default_tool: Tool::Cachegrind,
        })
    }

    #[cfg(not(feature = "cachegrind"))]
    pub fn new(
        config: Option<crate::__internal::InternalLibraryBenchmarkConfig>,
        args: Vec<String>,
        has_setup: bool,
        has_teardown: bool,
    ) -> Self {
        Self(crate::__internal::InternalLibraryBenchmarkGroups {
            config: config.unwrap_or_default(),
            groups: Vec::default(),
            command_line_args: args,
            has_setup,
            has_teardown,
            default_tool: Tool::Callgrind,
        })
    }

    pub fn add_group(
        &mut self,
        id: String,
        config: Option<crate::__internal::InternalLibraryBenchmarkConfig>,
        compare_by_id: Option<bool>,
        max_parallel: Option<usize>,
        has_setup: bool,
        has_teardown: bool,
        benches: MacroLibBenches,
    ) {
        let mut internal_group = crate::__internal::InternalLibraryBenchmarkGroup {
            id,
            config,
            has_setup,
            has_teardown,
            compare_by_id,
            max_parallel,
            ..Default::default()
        };

        for (function_name, get_config, macro_lib_benches) in benches {
            let mut benches = crate::__internal::InternalLibraryBenchmarkBenches {
                benches: vec![],
                config: get_config(),
            };
            for macro_lib_bench in *macro_lib_benches {
                let bench = crate::__internal::InternalLibraryBenchmarkBench {
                    id: macro_lib_bench.id_display.map(str::to_owned),
                    args: macro_lib_bench.args_display.map(str::to_owned),
                    consts_display: macro_lib_bench.consts_display.map(str::to_owned),
                    function_name: (*function_name).to_owned(),
                    config: macro_lib_bench.config.map(|f| f()),
                    iter_count: match macro_lib_bench.func {
                        super::InternalLibFunctionKind::Iter(func) => {
                            Some(func(super::InternalBenchRunMode::Default, None))
                        }
                        super::InternalLibFunctionKind::Default(_) => None,
                    },
                };
                benches.benches.push(bench);
            }
            internal_group.library_benchmarks.push(benches);
        }

        self.0.groups.push(internal_group);
    }

    pub fn build(self) -> crate::__internal::InternalLibraryBenchmarkGroups {
        self.0
    }
}
