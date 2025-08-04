use std::ops::Deref;

use derive_more::{Deref as DerefDerive, DerefMut as DerefMutDerive};
use proc_macro2::TokenStream;
use proc_macro_error2::abort;
use quote::{format_ident, quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse2, parse_quote, Attribute, Expr, ExprPath, FnArg, Ident, ItemFn, MetaNameValue, Pat,
    PatIdent, PatType, Signature, Token,
};

use crate::common::{self, format_ident, truncate_str_utf8, BenchesArgs, File};
use crate::{defaults, CargoMetadata};

/// The benchmark mode for `iter` and any another option in the bench attributes
#[derive(Debug)]
enum BenchMode {
    Iter(Iter),
    Args(Args),
}

/// This struct reflects the `args` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct Args(common::Args);

/// This is the counterpart for the `#[bench]` attribute
///
/// The `#[benches]` attribute is also parsed into this structure.
#[derive(Debug)]
struct Bench {
    id: Ident,
    mode: BenchMode,
    config: BenchConfig,
    setup: Setup,
    teardown: Teardown,
}

#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct BenchConfig(common::BenchConfig);

#[derive(Debug, Clone, DerefDerive, DerefMutDerive)]
struct Callee<'a>(&'a Signature);

#[derive(Debug, Clone)]
struct Iter(Expr);

/// This is the counterpart to the `#[library_benchmark]` attribute.
#[derive(Debug, Default)]
struct LibraryBenchmark {
    config: LibraryBenchmarkConfig,
    setup: Setup,
    teardown: Teardown,
    benches: Vec<Bench>,
}

/// The `config` parameter of the `#[library_benchmark]` attribute
///
/// The `BenchConfig` and `LibraryBenchmarkConfig` are rendered differently, hence the different
/// structures
///
/// Note: This struct is completely independent of the `iai_callgrind::LibraryBenchmarkConfig`
/// struct with the same name.
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct LibraryBenchmarkConfig(common::BenchConfig);

#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct Setup(common::Setup);

#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct Teardown(common::Teardown);

impl ToTokens for Args {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl Bench {
    fn parse_bench_attribute(
        item_fn: &ItemFn,
        attr: &Attribute,
        id: Ident,
        other_setup: &Setup,
        other_teardown: &Teardown,
    ) -> syn::Result<Self> {
        let expected_num_args = item_fn.sig.inputs.len();
        let meta = attr.meta.require_list()?;

        let mut args = Args::default();
        let mut config = BenchConfig::default();
        let mut setup = Setup::default();
        let mut teardown = Teardown::default();

        if let Ok(pairs) =
            meta.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
        {
            for pair in pairs {
                if pair.path.is_ident("args") {
                    args.parse_pair(&pair)?;
                } else if pair.path.is_ident("config") {
                    config.parse_pair(&pair);
                } else if pair.path.is_ident("setup") {
                    setup.parse_pair(&pair);
                } else if pair.path.is_ident("teardown") {
                    teardown.parse_pair(&pair);
                } else {
                    abort!(
                        pair, "Invalid argument: {}", pair.path.require_ident()?;
                        help = "Valid arguments are: `args`, `config`, `setup`, teardown`"
                    );
                }
            }
        } else {
            args.parse_meta_list(meta)?;
        }

        setup.update(other_setup);
        teardown.update(other_teardown);

        args.check_num_arguments(expected_num_args, setup.is_some());

        Ok(Bench {
            id,
            mode: BenchMode::Args(args),
            config,
            setup,
            teardown,
        })
    }

    fn parse_benches_attribute(
        item_fn: &ItemFn,
        attr: &Attribute,
        id: &Ident,
        other_setup: &Setup,
        other_teardown: &Teardown,
        cargo_meta: Option<&CargoMetadata>,
    ) -> syn::Result<Vec<Self>> {
        let expected_num_args = item_fn.sig.inputs.len();
        let meta = attr.meta.require_list()?;

        let mut config = BenchConfig::default();
        let mut setup = Setup::default();
        let mut teardown = Teardown::default();
        let mut args = BenchesArgs::default();
        let mut file = File::default();
        let mut iter = common::Iter::default();

        if let Ok(pairs) =
            meta.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
        {
            for pair in pairs {
                if pair.path.is_ident("args") {
                    args.parse_pair(&pair)?;
                } else if pair.path.is_ident("config") {
                    config.parse_pair(&pair);
                } else if pair.path.is_ident("setup") {
                    setup.parse_pair(&pair);
                } else if pair.path.is_ident("teardown") {
                    teardown.parse_pair(&pair);
                } else if pair.path.is_ident("file") {
                    file.parse_pair(&pair)?;
                } else if pair.path.is_ident("iter") {
                    iter.parse_pair(&pair);
                } else {
                    abort!(
                        pair, "Invalid argument: {}", pair.path.require_ident()?;
                        help = "Valid arguments are: `args`, `file`, `iter`, `config`, `setup`, `teardown`"
                    );
                }
            }
        } else {
            args = BenchesArgs::from_meta_list(meta)?;
        }

        setup.update(other_setup);
        teardown.update(other_teardown);

        let benches = common::Bench::from_benches_attribute(
            id,
            args,
            &file,
            &iter,
            cargo_meta,
            setup.is_some(),
            expected_num_args,
        )
        .into_iter()
        .map(|b| Bench {
            id: b.id,
            mode: b.mode.into(),
            config: config.clone(),
            setup: setup.clone(),
            teardown: teardown.clone(),
        })
        .collect();

        Ok(benches)
    }

    #[allow(clippy::too_many_lines)]
    fn render_as_code(&self, callee: &Callee) -> TokenStream {
        let bench_id = &self.id;
        let elem_ident = format_ident!("__elem");
        let run_func_id = format_ident("__run", Some(bench_id));
        let callee_ident = &callee.ident;

        let func = match &self.mode {
            BenchMode::Iter(iter) => {
                let iter_expr = iter.expr();

                let index_ident = Iter::index_ident();
                let iter_ident = Iter::iter_ident();

                let len = callee.len_inputs();
                if len > 1 || len == 0 {
                    abort!(
                        iter_expr,
                        "The benchmark function can only take exactly one argument if the iter parameter is present";
                        help = "fn benchmark_function(arg: String) ...";
                        note = "If you need more than one argument you can use a tuple as input and
                        \ndestruct it in the function signature. Example:
                        \n
                        \n#[benches::some_id(iter = vec![(1, 2)])]
                        \nfn benchmark_function((first, second): (u64, u64)) -> usize { ... }"
                    )
                }

                let bench_id_func = callee.to_caller_signature(&elem_ident, bench_id);

                let (iter_count, iter_elem) = iter.render_as_code(&self.setup);
                let call_bench_func = quote_spanned! { callee_ident.span() =>
                    std::hint::black_box(
                        __iai_callgrind_wrapper_mod::#callee_ident(#elem_ident)
                    )
                };

                let call_bench_id = self
                    .teardown
                    .render_as_code(quote_spanned! { bench_id.span() => #bench_id(#elem_ident) });
                let export_name = format!("__iai_callgrind__{callee_ident}::{run_func_id}");
                let export = if cfg!(unsafe_keyword_needed) {
                    quote!(#[unsafe(export_name = #export_name)])
                } else {
                    quote!(#[export_name = #export_name])
                };

                quote!(
                   #[inline(never)]
                   #bench_id_func {
                       #call_bench_func
                   }
                   #[inline(never)]
                   #export
                   pub fn #run_func_id(#index_ident: Option<usize>) -> usize {
                       let #iter_ident = #iter_expr;

                       if let Some(#index_ident) = #index_ident {
                           #[allow(clippy::useless_conversion)]
                           let #elem_ident = #iter_elem;
                           let _ = #call_bench_id;
                           0
                       } else {
                           #[allow(clippy::useless_conversion)]
                           #[allow(clippy::iter_count)]
                           #iter_count
                       }
                   }
                )
            }
            BenchMode::Args(args) => {
                let inner = self.setup.render_as_code(args);
                let call_bench_id = if self.setup.is_some() {
                    self.teardown.render_as_code(quote_spanned! {
                        bench_id.span() => {
                            let __setup = #inner;
                            std::hint::black_box(#bench_id(__setup))
                        }
                    })
                } else {
                    self.teardown.render_as_code(
                        quote_spanned! { bench_id.span() => std::hint::black_box(#bench_id(#inner))
                        },
                    )
                };

                let (call_bench_func, bench_id_func) = if self.setup.is_some() {
                    let call_bench_func = quote_spanned! { callee_ident.span() =>
                        std::hint::black_box(
                            __iai_callgrind_wrapper_mod::#callee_ident(#elem_ident)
                        )
                    };
                    let bench_id_func = callee.to_caller_signature(&elem_ident, bench_id);

                    (call_bench_func, bench_id_func)
                } else {
                    match callee.input_ids_no_wildcards() {
                        Ok(inputs) => {
                            let call_bench_func = quote_spanned! { callee_ident.span() =>
                                std::hint::black_box(
                                    __iai_callgrind_wrapper_mod::#callee_ident(#(#inputs),*)
                                )
                            };
                            let bench_id_func = callee.with_id(bench_id.clone());

                            (call_bench_func, bench_id_func)
                        }
                        Err(message) => abort!(callee, "{}", message),
                    }
                };

                let export_name = format!("__iai_callgrind__{callee_ident}::{run_func_id}");

                let export = if cfg!(unsafe_keyword_needed) {
                    quote!(#[unsafe(export_name = #export_name)])
                } else {
                    quote!(#[export_name = #export_name])
                };

                quote!(
                   #[inline(never)]
                   #bench_id_func {
                       #call_bench_func
                   }
                   #[inline(never)]
                   #export
                   pub fn #run_func_id() {
                       let _ = #call_bench_id;
                   }
                )
            }
        };

        let config = self.config.render_as_code(bench_id);
        quote! {
            #config
            #func
        }
    }

    fn render_as_member(&self) -> TokenStream {
        let id = &self.id;
        let id_display = self.id.to_string();
        let config = self.config.render_as_member(id);
        let run_id = format_ident("__run", Some(id));

        match &self.mode {
            BenchMode::Iter(iter) => {
                let args_string = self.setup.to_string_with_iter(&iter.0);
                let args_display = truncate_str_utf8(&args_string, defaults::MAX_BYTES_ARGS);
                quote! {
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: Some(#id_display),
                        args_display: Some(#args_display),
                        func: iai_callgrind::__internal::InternalLibFunctionKind::Iter(#run_id),
                        config: #config
                    }
                }
            }
            BenchMode::Args(args) => {
                let args_string = self.setup.to_string_with_args(args);
                let args_display = truncate_str_utf8(&args_string, defaults::MAX_BYTES_ARGS);
                quote! {
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: Some(#id_display),
                        args_display: Some(#args_display),
                        func: iai_callgrind::__internal::InternalLibFunctionKind::Default(#run_id),
                        config: #config
                    }
                }
            }
        }
    }
}

impl BenchConfig {
    pub fn render_as_code(&self, id: &Ident) -> TokenStream {
        if let Some(config) = &self.deref().0 {
            let ident = common::BenchConfig::ident(id);
            quote! {
                #[inline(never)]
                pub fn #ident() -> iai_callgrind::__internal::InternalLibraryBenchmarkConfig {
                    #config.into()
                }
            }
        } else {
            TokenStream::new()
        }
    }

    pub fn render_as_member(&self, id: &Ident) -> TokenStream {
        if self.deref().0.is_some() {
            let ident = common::BenchConfig::ident(id);
            quote! { Some(#ident) }
        } else {
            quote! { None }
        }
    }
}

impl From<common::BenchMode> for BenchMode {
    fn from(value: common::BenchMode) -> Self {
        match value {
            common::BenchMode::Iter(expr) => BenchMode::Iter(Iter(expr)),
            common::BenchMode::Args(args) => BenchMode::Args(Args(args)),
        }
    }
}

impl Callee<'_> {
    fn len_inputs(&self) -> usize {
        self.0.inputs.len()
    }

    fn to_caller_signature(&self, elem_ident: &Ident, bench_id: &Ident) -> Signature {
        let fn_arg = self.0.inputs.iter().next().and_then(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_) => {
                abort!(fn_arg, "Methods with `self` are not allowed")
            }
            syn::FnArg::Typed(pat_type) => {
                let pat_type = PatType {
                    pat: Box::new(Pat::Ident(PatIdent {
                        ident: elem_ident.clone(),
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        subpat: None,
                    })),
                    ..pat_type.clone()
                };
                Some(FnArg::Typed(pat_type))
            }
        });

        Signature {
            ident: bench_id.clone(),
            inputs: fn_arg.map_or_else(Punctuated::new, |fn_arg| {
                let mut punct = Punctuated::new();
                punct.push_value(fn_arg);
                punct
            }),
            ..self.0.clone()
        }
    }

    fn with_id(&self, ident: Ident) -> Signature {
        Signature {
            ident,
            ..self.0.clone()
        }
    }

    fn input_ids_no_wildcards(&self) -> Result<Vec<Ident>, String> {
        self.0
            .inputs
            .iter()
            .map(|fn_arg| match fn_arg {
                FnArg::Receiver(_) => Err("Methods with `self` are not allowed".to_owned()),
                FnArg::Typed(pat_type) => match &*pat_type.pat {
                    Pat::Ident(pat_ident) => Ok(pat_ident.ident.clone()),
                    Pat::Wild(_) => Err("Wildcard patterns in the benchmark function signature \
                                         are unsupported"
                        .to_owned()),
                    _ => Err(
                        "Unsupported pattern. If you think this is an error please open an issue"
                            .to_owned(),
                    ),
                },
            })
            .collect::<Result<Vec<Ident>, String>>()
    }
}

impl Iter {
    fn iter_ident() -> Ident {
        format_ident!("__iter")
    }

    fn index_ident() -> Ident {
        format_ident!("__index")
    }

    fn expr(&self) -> &Expr {
        &self.0
    }

    fn render_as_code(&self, setup: &Setup) -> (TokenStream, TokenStream) {
        let iter_span = self.0.span();
        let iter_ident = Self::iter_ident();
        let index_ident = Self::index_ident();

        let iter_count = quote_spanned! { iter_span => #iter_ident.into_iter().count() };
        let iter_elem = if let Some(setup) = setup.expr() {
            quote_spanned! { setup.span() =>
            #iter_ident
                .into_iter()
                .nth(#index_ident)
                .map(#setup)
                .expect("The iterator index should be withing bounds")
            }
        } else {
            quote_spanned! { iter_span =>
                #iter_ident
                    .into_iter()
                    .nth(#index_ident)
                    .expect("The iterator index should be within bounds")
            }
        };

        (iter_count, iter_elem)
    }
}

impl LibraryBenchmark {
    fn extract_benches(
        &mut self,
        item_fn: &ItemFn,
        cargo_meta: Option<&CargoMetadata>,
    ) -> syn::Result<()> {
        let bench: syn::PathSegment = parse_quote!(bench);
        let benches: syn::PathSegment = parse_quote!(benches);

        for attr in &item_fn.attrs {
            let mut path_segments = attr.path().segments.iter();
            match path_segments.next() {
                Some(segment) if segment == &bench => {
                    if attr.path().segments.len() > 2 {
                        abort!(
                            attr, "Only one id is allowed";
                            help = "bench followed by :: and a single unique id";
                            note = r#"#[bench::my_id()] or #[bench::my_id("with", "args")]
                        or #[bench::my_id(args = (arg1, ...), config = ...)]"#
                        );
                    }
                    let Some(id) = path_segments.next().map(|p| p.ident.clone()) else {
                        abort!(
                            attr, "An id is required";
                            help = "bench followed by :: and an unique id";
                            note = "#[bench::my_id(...)]"
                        );
                    };
                    self.benches.push(Bench::parse_bench_attribute(
                        item_fn,
                        attr,
                        id,
                        &self.setup,
                        &self.teardown,
                    )?);
                }
                Some(segment) if segment == &benches => {
                    if attr.path().segments.len() > 2 {
                        abort!(
                            attr, "Only one id is allowed";
                            help = "benches followed by :: and a single unique id";
                            note = r#"#[benches::my_id("with", "args")]
                        or #[benches::my_id(args = [arg1, ...]]"#
                        );
                    }
                    let Some(id) = path_segments.next().map(|p| p.ident.clone()) else {
                        abort!(
                            attr, "An id is required";
                            help = "benches followed by :: and an unique id";
                            note = "#[benches::my_id(...)]"
                        );
                    };
                    self.benches.extend(Bench::parse_benches_attribute(
                        item_fn,
                        attr,
                        &id,
                        &self.setup,
                        &self.teardown,
                        cargo_meta,
                    )?);
                }
                Some(segment) => {
                    abort!(
                        attr, "Invalid attribute: '{}'", segment.ident;
                        help = "Only the `bench` and the `benches` attribute are allowed";
                        note = r#"#[bench::my_id("with", "args")]
                    or #[benches::my_id(args = [("with", "args"), ...])]"#
                    );
                }
                None => {
                    // #[] => Syntax error: Expected an identifier
                    unreachable!("This case is handled by the compiler")
                }
            }
        }

        Ok(())
    }

    /// Render the `#[library_benchmark]` attribute when no outer attribute was present
    ///
    /// ```ignore
    /// #[library_benchmark]
    /// fn my_benchmark_function() -> u64 {
    ///     my_lib::bench_me(42)
    /// }
    /// ```
    fn render_standalone(self, item_fn: &ItemFn) -> TokenStream {
        let new_item_fn = create_item_fn(item_fn);

        let callee = Callee(&item_fn.sig);
        let callee_ident = &callee.ident;

        let elem_ident = format_ident!("__elem");
        let wrapper_ident = format_ident!("wrapper");
        let run_func_id = format_ident("__run", Some(&wrapper_ident));

        let config = self.config.render_as_code();

        let inner = self.setup.render_as_code(&Args::default());
        let call_wrapper = if self.setup.is_some() {
            self.teardown.render_as_code(quote_spanned! {
                wrapper_ident.span() => {
                    let __setup = #inner;
                    std::hint::black_box(#wrapper_ident(__setup))
                }
            })
        } else {
            self.teardown.render_as_code(quote_spanned! {
                wrapper_ident.span() =>
                    std::hint::black_box(#wrapper_ident(#inner))
            })
        };

        let (call_bench_func, wrapper_func) = if self.setup.is_some() {
            let call_bench_func = quote_spanned! { callee_ident.span() =>
                    std::hint::black_box(
                        __iai_callgrind_wrapper_mod::#callee_ident(#elem_ident)
                    )
            };
            let bench_id_func = callee.to_caller_signature(&elem_ident, &wrapper_ident);

            (call_bench_func, bench_id_func)
        } else {
            match callee.input_ids_no_wildcards() {
                Ok(inputs) => {
                    let call_bench_func = quote_spanned! { callee_ident.span() =>
                            std::hint::black_box(
                                __iai_callgrind_wrapper_mod::#callee_ident(#(#inputs),*)
                            )
                    };
                    let bench_id_func = callee.with_id(wrapper_ident.clone());

                    (call_bench_func, bench_id_func)
                }
                Err(message) => abort!(callee, "{}", message),
            }
        };

        let export_name = format!("__iai_callgrind__{callee_ident}::{run_func_id}");
        let export = if cfg!(unsafe_keyword_needed) {
            quote!(#[unsafe(export_name = #export_name)])
        } else {
            quote!(#[export_name = #export_name])
        };
        let func = quote! {
            iai_callgrind::__internal::InternalLibFunctionKind::Default(#run_func_id)
        };

        quote! {
            pub mod #callee_ident {
                use super::*;

                mod __iai_callgrind_wrapper_mod {
                    use super::*;

                    #[inline(never)]
                    #new_item_fn
                }

                pub const __BENCHES: &[iai_callgrind::__internal::InternalMacroLibBench]= &[
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: None,
                        args_display: None,
                        func: #func,
                        config: None
                    },
                ];

                #config

               #[inline(never)]
               #wrapper_func {
                   #call_bench_func
               }

               #[inline(never)]
               #export
               pub fn #run_func_id() {
                   let _ = #call_wrapper;
               }
            }
        }
    }

    /// Render the `#[library_benchmark]` when other outer attributes like `#[bench]` were present
    ///
    /// We use the function name of the annotated function as module name. This new module
    /// encloses the new functions generated from the `#[bench]` and `#[benches]` attribute as well
    /// as the original and unmodified benchmark function.
    ///
    /// The original benchmark function receives additional attributes `#[inline(never)]` to prevent
    /// the compiler from inlining this function. The benchmark function is wrapped into a module
    /// with a constant name. The main problem is that the compiler replaces functions with
    /// identical body. For example the functions
    ///
    /// ```ignore
    /// #[library_benchmark]
    /// #[bench::my_id(42)]
    /// fn my_bench(arg: u64) -> u64 {
    ///     my_lib::bench_me()
    /// }
    ///
    /// #[library_benchmark]
    /// #[bench::my_id(84)]
    /// fn my_bench_with_longer_function_name(arg: u64) -> u64 {
    ///     my_lib::bench_me()
    /// }
    /// ```
    ///
    /// would be treated by the compiler as a single function (it takes the one with the shorter
    /// function name, here `my_bench`) and both function names would be exported under the same
    /// name. If we don't export these functions with a common and constant module name in it, we
    /// wouldn't be able to match for
    /// `my_bench_with_longer_function_name::my_bench_with_longer_function_name` since this function
    /// was replaced by the compiler with `my_bench::my_bench`.
    ///
    /// Next, we store all necessary information in a `BENCHES` slice of
    /// `iai_callgrind::__internal::InternalMacroLibBench` structs. This slice can be easily
    /// accessed by the macros of the `iai-callgrind` package in which we finally can call all the
    /// benchmark functions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[library_benchmark]
    /// #[bench::my_id(42)]
    /// fn my_benchmark_function(arg: u64) -> u64 {
    ///     my_lib::bench_me(arg)
    /// }
    /// ```
    fn render_benches(self, item_fn: &ItemFn) -> TokenStream {
        let new_item_fn = create_item_fn(item_fn);

        let mod_name = &item_fn.sig.ident;
        let mut funcs = TokenStream::new();
        let mut lib_benches = vec![];
        for bench in self.benches {
            funcs.append_all(bench.render_as_code(&Callee(&item_fn.sig)));
            lib_benches.push(bench.render_as_member());
        }

        let config = self.config.render_as_code();
        quote! {
            pub mod #mod_name {
                use super::*;

                mod __iai_callgrind_wrapper_mod {
                    use super::*;

                    #[inline(never)]
                    #new_item_fn
                }

                pub const __BENCHES: &[iai_callgrind::__internal::InternalMacroLibBench] = &[
                    #(#lib_benches,)*
                ];

                #config

                #funcs
            }
        }
    }
}

impl Parse for LibraryBenchmark {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(Self::default())
        } else {
            let mut config = LibraryBenchmarkConfig::default();
            let mut setup = Setup::default();
            let mut teardown = Teardown::default();

            let pairs = input.parse_terminated(MetaNameValue::parse, Token![,])?;
            for pair in pairs {
                if pair.path.is_ident("config") {
                    config.parse_pair(&pair);
                } else if pair.path.is_ident("setup") {
                    setup.parse_pair(&pair);
                } else if pair.path.is_ident("teardown") {
                    teardown.parse_pair(&pair);
                } else {
                    abort!(
                        pair, "Invalid argument: {}", pair.path.require_ident()?;
                        help = "Valid arguments are: `config`, `setup`, `teardown`"
                    );
                }
            }

            let library_benchmark = LibraryBenchmark {
                config,
                setup,
                teardown,
                benches: vec![],
            };
            Ok(library_benchmark)
        }
    }
}

impl LibraryBenchmarkConfig {
    fn ident() -> Ident {
        format_ident("__get_config", None)
    }

    fn render_as_code(&self) -> TokenStream {
        let ident = Self::ident();
        if let Some(config) = &self.deref().0 {
            quote!(
                #[inline(never)]
                pub fn #ident()
                    -> Option<iai_callgrind::__internal::InternalLibraryBenchmarkConfig>
                {
                    Some(#config.into())
                }
            )
        } else {
            quote!(
                #[inline(never)]
                pub fn #ident()
                -> Option<iai_callgrind::__internal::InternalLibraryBenchmarkConfig> {
                    None
                }
            )
        }
    }
}

impl Setup {
    fn is_some(&self) -> bool {
        self.0 .0.is_some()
    }

    fn expr(&self) -> Option<&ExprPath> {
        self.0 .0.as_ref()
    }

    fn render_as_code(&self, args: &Args) -> TokenStream {
        if let Some(setup) = &self.deref().0 {
            quote_spanned! { setup.span() => std::hint::black_box(#setup(#args)) }
        } else {
            quote! { #args }
        }
    }
}

impl Teardown {
    fn render_as_code(&self, tokens: TokenStream) -> TokenStream {
        if let Some(teardown) = &self.deref().0 {
            quote_spanned! { teardown.span() => {
                    let __result = #tokens;
                    std::hint::black_box(#teardown(__result))
                }
            }
        } else {
            tokens
        }
    }
}

#[cfg(feature = "cachegrind")]
fn create_item_fn(item_fn: &ItemFn) -> ItemFn {
    let vis = parse_quote! { pub(super) };
    let item_fn_block = item_fn.block.clone();
    let block = parse_quote!(
        {
            iai_callgrind::client_requests::cachegrind::start_instrumentation();
            let __r = #item_fn_block;
            iai_callgrind::client_requests::cachegrind::stop_instrumentation();
            __r
        }
    );
    ItemFn {
        attrs: vec![],
        vis,
        sig: item_fn.sig.clone(),
        block,
    }
}

#[cfg(not(feature = "cachegrind"))]
fn create_item_fn(item_fn: &ItemFn) -> ItemFn {
    let vis = parse_quote! { pub(super) };
    ItemFn {
        attrs: vec![],
        vis,
        sig: item_fn.sig.clone(),
        block: item_fn.block.clone(),
    }
}

pub fn render(args: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let mut library_benchmark = parse2::<LibraryBenchmark>(args)?;
    let item_fn = parse2::<ItemFn>(input)?;

    let cargo_meta = CargoMetadata::try_new();

    library_benchmark.extract_benches(&item_fn, cargo_meta.as_ref())?;
    if library_benchmark.benches.is_empty() {
        Ok(library_benchmark.render_standalone(&item_fn))
    } else {
        Ok(library_benchmark.render_benches(&item_fn))
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::format_ident;
    use syn::{Expr, ExprStruct, ItemMod};

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Model {
        item: ItemMod,
    }

    impl Parse for Model {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            Ok(Self {
                item: input.parse::<ItemMod>()?,
            })
        }
    }

    fn expected_model(
        func: &ItemFn,
        benches: &[ExprStruct],
        get_config: Option<&Expr>,
        get_config_bench: &[(Ident, Expr)],
        bench: &[(Ident, Vec<Expr>)],
    ) -> Model {
        let callee = &func.sig.ident;

        let visibility = parse_quote! { pub(super) };
        let new_item_fn = ItemFn {
            attrs: vec![],
            vis: visibility,
            sig: func.sig.clone(),
            block: func.block.clone(),
        };

        let rendered_get_config = if let Some(expr) = get_config {
            quote!(
                #[inline(never)]
                pub fn __get_config()
                -> Option<iai_callgrind::__internal::InternalLibraryBenchmarkConfig>
                {
                    Some(#expr.into())
                }
            )
        } else {
            quote!(
                #[inline(never)]
                pub fn __get_config(
                ) -> Option<iai_callgrind::__internal::InternalLibraryBenchmarkConfig>
                {
                    None
                }
            )
        };
        let mut rendered_benches = vec![];
        for (ident, args) in bench {
            let config = get_config_bench.iter().find_map(|(i, expr)| {
                (i == ident).then(|| {
                    let ident = format_ident!("__get_config_{}", i);
                    quote!(
                        #[inline(never)]
                        pub fn #ident() ->
                            iai_callgrind::__internal::InternalLibraryBenchmarkConfig {
                                #expr.into()
                        }
                    )
                })
            });
            if let Some(config) = config {
                rendered_benches.push(config);
            }
            rendered_benches.push(quote!(
                #[inline(never)]
                pub fn #ident() {
                    let _ = std::hint::black_box(__iai_callgrind_wrapper_mod::#callee(
                        #(std::hint::black_box(#args)),*
                    ));
                }
            ));
        }
        parse_quote!(
            pub mod #callee {
                use super::*;

                mod __iai_callgrind_wrapper_mod {
                    use super::*;

                    #[inline(never)]
                    #new_item_fn
                }

                pub const __BENCHES: &[iai_callgrind::__internal::InternalMacroLibBench]= &[
                    #(#benches),*,
                ];

                #rendered_get_config

                #(#rendered_benches)*
            }
        )
    }

    #[test]
    fn test_only_library_benchmark_attribute() {
        let input = quote!(
            fn some() -> u8 {
                1 + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some() -> u8 {
                    1 + 2
                }
            ),
            &[parse_quote!(
                iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: None,
                    args_display: None,
                    func: wrapper,
                    config: None
                }
            )],
            None,
            &[],
            &[(parse_quote!(wrapper), vec![])],
        );
        let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_only_library_benchmark_attribute_with_config() {
        let input = quote!(
            fn some() -> u8 {
                1 + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some() -> u8 {
                    1 + 2
                }
            ),
            &[parse_quote!(
                iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: None,
                    args_display: None,
                    func: wrapper,
                    config: None
                }
            )],
            Some(&parse_quote!(LibraryBenchmarkConfig::default())),
            &[],
            &[(parse_quote!(wrapper), vec![])],
        );
        let actual: Model =
            parse2(render(quote!(config = LibraryBenchmarkConfig::default()), input).unwrap())
                .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bench_when_func_no_arg() {
        for attribute in [
            quote!(bench::my_id()),
            quote!(bench::my_id(args = ())),
            quote!(bench::my_id(args = [])),
        ] {
            dbg!(&attribute);
            let input = quote!(
                #[#attribute]
                fn some() -> u8 {
                    1 + 2
                }
            );

            let expected = expected_model(
                &parse_quote!(
                    fn some() -> u8 {
                        1 + 2
                    }
                ),
                &[parse_quote!(
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: Some("my_id"),
                        args_display: Some(""),
                        func: my_id,
                        config: None
                    }
                )],
                None,
                &[],
                &[(parse_quote!(my_id), vec![])],
            );
            let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_bench_when_func_one_arg() {
        for attribute in [
            quote!(bench::my_id(1)),
            quote!(bench::my_id(args = (1,))),
            quote!(bench::my_id(args = (1))),
            quote!(bench::my_id(args = [1])),
        ] {
            dbg!(&attribute);
            let input = quote!(
                #[#attribute]
                fn some(var: u8) -> u8 {
                    var + 2
                }
            );

            let expected = expected_model(
                &parse_quote!(
                    fn some(var: u8) -> u8 {
                        var + 2
                    }
                ),
                &[parse_quote!(
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: Some("my_id"),
                        args_display: Some("1"),
                        func: my_id,
                        config: None
                    }
                )],
                None,
                &[],
                &[(parse_quote!(my_id), vec![parse_quote!(1)])],
            );
            let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_bench_when_func_two_args() {
        for attribute in [
            quote!(bench::my_id(1, 2)),
            quote!(bench::my_id(args = (1, 2))),
            quote!(bench::my_id(args = [1, 2])),
        ] {
            dbg!(&attribute);
            let input = quote!(
                #[#attribute]
                fn some(one: u8, two: u8) -> u8 {
                    one + two
                }
            );

            let expected = expected_model(
                &parse_quote!(
                    fn some(one: u8, two: u8) -> u8 {
                        one + two
                    }
                ),
                &[parse_quote!(
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: Some("my_id"),
                        args_display: Some("1 , 2"),
                        func: my_id,
                        config: None
                    }
                )],
                None,
                &[],
                &[(parse_quote!(my_id), vec![parse_quote!(1), parse_quote!(2)])],
            );
            let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_bench_when_config_no_args() {
        for attribute in [
            quote!(bench::my_id(config = LibraryBenchmarkConfig::default())),
            quote!(bench::my_id(
                args = (),
                config = LibraryBenchmarkConfig::default()
            )),
        ] {
            dbg!(&attribute);
            let input = quote!(
                #[#attribute]
                fn some() -> u8 {
                    1 + 2
                }
            );

            let expected = expected_model(
                &parse_quote!(
                    fn some() -> u8 {
                        1 + 2
                    }
                ),
                &[parse_quote!(
                    iai_callgrind::__internal::InternalMacroLibBench {
                        id_display: Some("my_id"),
                        args_display: Some(""),
                        func: my_id,
                        config: Some(__get_config_my_id)
                    }
                )],
                None,
                &[(
                    parse_quote!(my_id),
                    parse_quote!(LibraryBenchmarkConfig::default()),
                )],
                &[(parse_quote!(my_id), vec![])],
            );
            let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_bench_when_config_and_library_benchmark_config() {
        let attribute = quote!(bench::my_id(config = LibraryBenchmarkConfig::default()));
        dbg!(&attribute);
        let input = quote!(
            #[#attribute]
            fn some() -> u8 {
                1 + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some() -> u8 {
                    1 + 2
                }
            ),
            &[parse_quote!(
                iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("my_id"),
                    args_display: Some(""),
                    func: my_id,
                    config: Some(__get_config_my_id)
                }
            )],
            Some(&parse_quote!(LibraryBenchmarkConfig::new())),
            &[(
                parse_quote!(my_id),
                parse_quote!(LibraryBenchmarkConfig::default()),
            )],
            &[(parse_quote!(my_id), vec![])],
        );
        let actual: Model =
            parse2(render(quote!(config = LibraryBenchmarkConfig::new()), input).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bench_when_multiple_no_args() {
        let input = quote!(
            #[bench::first()]
            #[bench::second()]
            fn some() -> u8 {
                1 + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some() -> u8 {
                    1 + 2
                }
            ),
            &[
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("first"),
                    args_display: Some(""),
                    func: first,
                    config: None
                }),
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("second"),
                    args_display: Some(""),
                    func: second,
                    config: None
                }),
            ],
            None,
            &[],
            &[
                (parse_quote!(first), vec![]),
                (parse_quote!(second), vec![]),
            ],
        );
        let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bench_when_multiple_one_arg() {
        let input = quote!(
            #[bench::first(1)]
            #[bench::second(2)]
            fn some(var: u8) -> u8 {
                var + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some(var: u8) -> u8 {
                    var + 2
                }
            ),
            &[
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("first"),
                    args_display: Some("1"),
                    func: first,
                    config: None
                }),
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("second"),
                    args_display: Some("2"),
                    func: second,
                    config: None
                }),
            ],
            None,
            &[],
            &[
                (parse_quote!(first), vec![parse_quote!(1)]),
                (parse_quote!(second), vec![parse_quote!(2)]),
            ],
        );
        let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bench_when_multiple_with_config_first() {
        let input = quote!(
            #[bench::first(args = (1), config = LibraryBenchmarkConfig::default())]
            #[bench::second(2)]
            fn some(var: u8) -> u8 {
                var + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some(var: u8) -> u8 {
                    var + 2
                }
            ),
            &[
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("first"),
                    args_display: Some("1"),
                    func: first,
                    config: Some(__get_config_first)
                }),
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("second"),
                    args_display: Some("2"),
                    func: second,
                    config: None
                }),
            ],
            None,
            &[(
                parse_quote!(first),
                parse_quote!(LibraryBenchmarkConfig::default()),
            )],
            &[
                (parse_quote!(first), vec![parse_quote!(1)]),
                (parse_quote!(second), vec![parse_quote!(2)]),
            ],
        );
        let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bench_when_multiple_with_config_second() {
        let input = quote!(
            #[bench::first(1)]
            #[bench::second(args = (2), config = LibraryBenchmarkConfig::default())]
            fn some(var: u8) -> u8 {
                var + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some(var: u8) -> u8 {
                    var + 2
                }
            ),
            &[
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("first"),
                    args_display: Some("1"),
                    func: first,
                    config: None
                }),
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("second"),
                    args_display: Some("2"),
                    func: second,
                    config: Some(__get_config_second)
                }),
            ],
            None,
            &[(
                parse_quote!(second),
                parse_quote!(LibraryBenchmarkConfig::default()),
            )],
            &[
                (parse_quote!(first), vec![parse_quote!(1)]),
                (parse_quote!(second), vec![parse_quote!(2)]),
            ],
        );
        let actual: Model = parse2(render(quote!(), input).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bench_when_multiple_with_config_all() {
        let input = quote!(
            #[bench::first(args = (1), config = LibraryBenchmarkConfig::new())]
            #[bench::second(args = (2), config = LibraryBenchmarkConfig::default())]
            fn some(var: u8) -> u8 {
                var + 2
            }
        );

        let expected = expected_model(
            &parse_quote!(
                fn some(var: u8) -> u8 {
                    var + 2
                }
            ),
            &[
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("first"),
                    args_display: Some("1"),
                    func: first,
                    config: Some(__get_config_first)
                }),
                parse_quote!(iai_callgrind::__internal::InternalMacroLibBench {
                    id_display: Some("second"),
                    args_display: Some("2"),
                    func: second,
                    config: Some(__get_config_second)
                }),
            ],
            Some(&parse_quote!(LibraryBenchmarkConfig::does_not_exist())),
            &[
                (
                    parse_quote!(first),
                    parse_quote!(LibraryBenchmarkConfig::new()),
                ),
                (
                    parse_quote!(second),
                    parse_quote!(LibraryBenchmarkConfig::default()),
                ),
            ],
            &[
                (parse_quote!(first), vec![parse_quote!(1)]),
                (parse_quote!(second), vec![parse_quote!(2)]),
            ],
        );

        let actual: Model = parse2(
            render(
                quote!(config = LibraryBenchmarkConfig::does_not_exist()),
                input,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}
