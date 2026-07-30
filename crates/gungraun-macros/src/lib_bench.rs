use std::ops::Deref;

use derive_more::{Deref as DerefDerive, DerefMut as DerefMutDerive};
use proc_macro_error3::abort;
use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, format_ident, quote, quote_spanned};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Expr, ExprPath, FnArg, Generics, Ident, ItemFn, MetaNameValue, Pat, PatType,
    Signature, Token, Type, parse_quote, parse_quote_spanned, parse2,
};

use crate::common::{
    self, BenchesArgs, BenchesConsts, File, format_ident, pattern_to_single_function_ident,
    truncate_str_utf8,
};
use crate::{CargoMetadata, defaults};

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
    config: BenchConfig,
    consts: Consts,
    generics: Generics,
    id: Ident,
    mode: BenchMode,
    output_type: Option<Type>,
    setup: Setup,
    teardown: Teardown,
}

#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct BenchConfig(common::BenchConfig);

#[derive(Debug, Clone, DerefDerive, DerefMutDerive)]
struct Callee<'a>(&'a Signature);

/// This struct reflects the `consts` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct Consts(common::Consts);

#[derive(Debug, Clone)]
struct Iter(Expr);

/// This is the counterpart to the `#[library_benchmark]` attribute.
#[derive(Debug, Default)]
struct LibraryBenchmark {
    benches: Vec<Bench>,
    config: LibraryBenchmarkConfig,
    setup: Setup,
    teardown: Teardown,
}

/// The `config` parameter of the `#[library_benchmark]` attribute
///
/// The `BenchConfig` and `LibraryBenchmarkConfig` are rendered differently, hence the different
/// structures
///
/// Note: This struct is completely independent of the `gungraun::LibraryBenchmarkConfig`
/// struct with the same name.
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct LibraryBenchmarkConfig(common::BenchConfig);

struct PerfRenderer<'a> {
    has_generics: bool,
    input_types: &'a [Type],
    output_type: Option<&'a Type>,
    setup: &'a Setup,
    shim_func_call: &'a TokenStream,
    shim_mod: &'a Ident,
    single_input_type: Option<&'a Type>,
    teardown: &'a Teardown,
}

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
        let expected_num_consts = item_fn.sig.generics.const_params().count();
        let generics = item_fn.sig.generics.clone();

        let meta = attr.meta.require_list()?;

        let mut args = Args::default();
        let mut consts = Consts::default();
        let mut config = BenchConfig::default();
        let mut setup = Setup::default();
        let mut teardown = Teardown::default();

        match meta.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated) {
            Ok(pairs) => {
                for pair in pairs {
                    if pair.path.is_ident("args") {
                        args.parse_pair(&pair)?;
                    } else if pair.path.is_ident("consts") {
                        consts.parse_pair(&pair)?;
                    } else if pair.path.is_ident("config") {
                        config.parse_pair(&pair);
                    } else if pair.path.is_ident("setup") {
                        setup.parse_pair(&pair);
                    } else if pair.path.is_ident("teardown") {
                        teardown.parse_pair(&pair);
                    } else {
                        abort!(
                            pair, "Invalid parameter: {}", pair.path.require_ident()?;
                            help = "Valid parameters are: `args`, `consts`, `config`, \
                            `setup`, `teardown`"
                        );
                    }
                }
            }
            _ => {
                args.parse_meta_list(meta)?;
            }
        }

        setup.update(other_setup);
        teardown.update(other_teardown);

        args.check_num_arguments(expected_num_args, setup.is_some());
        consts.check_num_arguments(expected_num_consts);

        let output_type = match &item_fn.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some(*ty.clone()),
        };
        Ok(Self {
            id,
            mode: BenchMode::Args(args),
            config,
            setup,
            teardown,
            consts,
            generics,
            output_type,
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
        let expected_num_consts = item_fn.sig.generics.const_params().count();
        let generics = item_fn.sig.generics.clone();
        let meta = attr.meta.require_list()?;

        let mut config = BenchConfig::default();
        let mut setup = Setup::default();
        let mut teardown = Teardown::default();
        let mut args = BenchesArgs::new(expected_num_args);
        let mut file = File::default();
        let mut iter = common::Iter::default();
        let mut consts = BenchesConsts::new(expected_num_consts);

        match meta.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated) {
            Ok(pairs) => {
                for pair in pairs {
                    if pair.path.is_ident("args") {
                        args.parse_pair(&pair)?;
                    } else if pair.path.is_ident("consts") {
                        consts.parse_pair(&pair)?;
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
                            pair, "Invalid parameter: {}", pair.path.require_ident()?;
                            help = "Valid parameters are: `args`, `consts`, `file`, `iter`, \
                            `config`, `setup`, `teardown`"
                        );
                    }
                }
            }
            _ => {
                args = BenchesArgs::from_meta_list(meta, expected_num_args)?;
            }
        }

        setup.update(other_setup);
        teardown.update(other_teardown);

        let benches = common::Bench::from_benches_attribute(
            item_fn.sig.ident.span(),
            id,
            args,
            consts,
            &file,
            &iter,
            cargo_meta,
            setup.is_some(),
        )
        .into_iter()
        .map(|b| {
            let output_type = match &item_fn.sig.output {
                syn::ReturnType::Default => None,
                syn::ReturnType::Type(_, ty) => Some(*ty.clone()),
            };
            Self {
                id: b.id,
                mode: b.mode.into(),
                config: config.clone(),
                setup: setup.clone(),
                teardown: teardown.clone(),
                consts: b.consts.map_or_else(Consts::default, Into::into),
                generics: generics.clone(),
                output_type,
            }
        })
        .collect();

        Ok(benches)
    }

    #[expect(clippy::too_many_lines)]
    fn render_as_code(&self, callee: &Callee) -> TokenStream {
        let bench_id = &self.id;
        let elem_ident = format_ident!("__elem");
        let run_func_ident = format_ident("__run", Some(bench_id));
        let callee_ident = &callee.ident;
        let count_ident = format_ident!("__count");

        let bench_wrapper_mod = format_ident!("__gungraun_wrapper_mod");
        let shim_mod = format_ident("__gungraun_wrapper_id_mod", Some(bench_id));
        let (shim_func, pats) = callee.to_caller_signature(&elem_ident, bench_id);

        let input_types = callee.input_types();
        let single_input_type = callee.single_input_type();
        let (bench_func_call, shim_func_call) =
            self.consts
                .to_function_calls(&self.generics, callee_ident, Some(bench_id));

        let perf_renderer = PerfRenderer {
            has_generics: !self.generics.params.is_empty(),
            input_types: &input_types,
            output_type: self.output_type.as_ref(),
            setup: &self.setup,
            shim_func_call: &shim_func_call,
            shim_mod: &shim_mod,
            single_input_type: single_input_type.as_ref(),
            teardown: &self.teardown,
        };

        let func = match &self.mode {
            // The amount of input arguments of the benchmark function is already verified to be
            // exactly one
            BenchMode::Iter(iter) => {
                let iter_expr = iter.expr();

                let index_ident = Iter::index_ident();
                let iter_ident = Iter::iter_ident();

                let (iter_count, iter_elem) =
                    iter.render_as_code(&self.setup, &elem_ident, single_input_type.as_ref());

                let call_bench_func = quote_spanned! { callee_ident.span() =>
                    std::hint::black_box(
                        #bench_wrapper_mod::#bench_func_call(#(#pats),*)
                    )
                };

                let shim_call = perf_renderer.render_shim_call(&quote!(#elem_ident));
                let call_shim_func = self.teardown.render_as_code(quote_spanned! {
                    bench_id.span() => std::hint::black_box(#shim_call)
                });

                let run_perf_dynamic = perf_renderer.render_iter_batch(iter, None);

                let run_perf_repeat = perf_renderer.render_iter_batch(iter, Some(&count_ident));

                let run_perf_overhead =
                    perf_renderer.render_iter_overhead_batch(iter, &count_ident);

                let run_perf_once = perf_renderer.render_iter_once(iter);

                quote!(
                    mod #shim_mod {
                        use super::*;
                        #[inline(never)]
                        pub(super) #shim_func {
                            #call_bench_func
                        }
                    }

                    #[inline(never)]
                    pub fn #run_func_ident(
                        mode: gungraun::__internal::InternalBenchRunMode,
                        #index_ident: Option<usize>,
                    ) -> usize {
                        let #iter_ident = #iter_expr;

                        if let Some(#index_ident) = #index_ident {
                            #[allow(clippy::let_unit_value)]
                            match mode {
                                gungraun::__internal::InternalBenchRunMode::Default => {
                                    #iter_elem
                                    let _ = #call_shim_func;
                                }
                                gungraun::__internal::InternalBenchRunMode::PerfDynamic => {
                                    let _ = #run_perf_dynamic;
                                }
                                gungraun::__internal::InternalBenchRunMode::PerfCalibrate => {
                                    gungraun::__internal::perf::calibrate();
                                }
                                gungraun::__internal::InternalBenchRunMode::PerfOverhead(
                                    #count_ident
                                ) => {
                                    let _ = #run_perf_overhead;
                                }
                                gungraun::__internal::InternalBenchRunMode::PerfRepeat(
                                    #count_ident
                                ) => {
                                    let _ = #run_perf_repeat;
                                }
                                gungraun::__internal::InternalBenchRunMode::PerfOnce => {
                                    let _ = #run_perf_once;
                                }
                            };
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
                let inner_without_black_box = self.setup.render_without_black_box(args);

                // There is a difference to allow the clippy let_unit_value lint
                let call_shim_func = if self.setup.is_some() {
                    let shim_call = perf_renderer.render_shim_call(&quote!(__setup));
                    // Specify the type early for better error messages
                    if let Some(input_type) = single_input_type.as_ref() {
                        self.teardown.render_as_code(quote_spanned! {
                            bench_id.span() => {
                                #[allow(clippy::let_unit_value)]
                                let __setup: #input_type = #inner_without_black_box;
                                let __setup = std::hint::black_box(__setup);
                                std::hint::black_box(#shim_call)
                            }
                        })
                    } else {
                        self.teardown.render_as_code(quote_spanned! {
                            bench_id.span() => {
                                #[allow(clippy::let_unit_value)]
                                let __setup = #inner;
                                std::hint::black_box(#shim_call)
                            }
                        })
                    }
                } else if self.generics.params.is_empty() && !input_types.is_empty() {
                    let tuple_ty = tuple_type(&input_types);
                    let arg_idents = (0..input_types.len())
                        .map(|index| format_ident!("__arg_{index}"))
                        .collect::<Vec<_>>();
                    let shim_call = perf_renderer.render_shim_call(&quote!(#(#arg_idents),*));

                    self.teardown
                        .render_as_code(quote_spanned! { bench_id.span() => {
                            #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                            let (#(#arg_idents),*,): #tuple_ty = (#inner,);

                            std::hint::black_box(#shim_call)
                        }})
                } else {
                    let shim_call = perf_renderer.render_shim_call(&quote!(#inner));
                    self.teardown
                        .render_as_code(quote_spanned! { bench_id.span() =>
                            std::hint::black_box(#shim_call)
                        })
                };

                let call_bench_func = quote_spanned! { callee_ident.span() =>
                        std::hint::black_box(
                            #bench_wrapper_mod::#bench_func_call(#(#pats),*)
                        )
                };

                let run_perf_dynamic =
                    perf_renderer.render_args_batch(args, &inner, &inner_without_black_box, None);

                let run_perf_repeat = perf_renderer.render_args_batch(
                    args,
                    &inner,
                    &inner_without_black_box,
                    Some(&count_ident),
                );

                let run_perf_overhead = perf_renderer.render_args_overhead_batch(
                    args,
                    &inner,
                    &inner_without_black_box,
                    &count_ident,
                );

                let run_perf_once =
                    perf_renderer.render_args_once(args, &inner, &inner_without_black_box);

                quote!(
                    mod #shim_mod {
                        use super::*;
                        #[inline(never)]
                        pub(super) #shim_func {
                           #call_bench_func
                        }
                    }

                    #[inline(never)]
                    pub fn #run_func_ident(mode: gungraun::__internal::InternalBenchRunMode) {
                        #[allow(clippy::let_unit_value)]
                        match mode {
                            gungraun::__internal::InternalBenchRunMode::Default => {
                                let _ = #call_shim_func;
                            }
                            gungraun::__internal::InternalBenchRunMode::PerfDynamic => {
                                let _ = #run_perf_dynamic;
                            }
                            gungraun::__internal::InternalBenchRunMode::PerfCalibrate => {
                                gungraun::__internal::perf::calibrate();
                            }
                            gungraun::__internal::InternalBenchRunMode::PerfOverhead(
                                #count_ident
                            ) => {
                                let _ = #run_perf_overhead;
                            }
                            gungraun::__internal::InternalBenchRunMode::PerfRepeat(
                                #count_ident
                            ) => {
                                let _ = #run_perf_repeat;
                            }
                            gungraun::__internal::InternalBenchRunMode::PerfOnce => {
                                let _ = #run_perf_once;
                            }
                        };
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

        let (args_string, func_kind) = match &self.mode {
            BenchMode::Iter(iter) => (
                self.setup.to_string_with_iter(&iter.0),
                quote! {Iter(#run_id)},
            ),
            BenchMode::Args(args) => (
                self.setup.to_string_with_args(args),
                quote! {Default(#run_id)},
            ),
        };
        let func = quote!(gungraun::__internal::InternalLibFunctionKind::#func_kind);

        let args_display = if args_string.is_empty() {
            quote! {None}
        } else {
            let display = truncate_str_utf8(&args_string, defaults::MAX_BYTES_ARGS);
            quote! {Some(#display)}
        };

        let consts_display = if let Some(consts_string) = self.consts.maybe_string() {
            let consts_display = truncate_str_utf8(&consts_string, defaults::MAX_BYTES_ARGS);
            quote! {Some(#consts_display)}
        } else {
            quote! {None}
        };

        quote! {
            gungraun::__internal::InternalMacroLibBench {
                id_display: Some(#id_display),
                args_display: #args_display,
                consts_display: #consts_display,
                func: #func,
                config: #config
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
                pub fn #ident() -> gungraun::__internal::InternalLibraryBenchmarkConfig {
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
            common::BenchMode::Iter(expr) => Self::Iter(Iter(expr)),
            common::BenchMode::Args(args) => Self::Args(Args(args)),
        }
    }
}

impl Callee<'_> {
    #[expect(unused)]
    fn len_inputs(&self) -> usize {
        self.0.inputs.len()
    }

    /// Convert to the function signature of the function calling the `Callee` (benchmark function)
    ///
    /// All elements with multiple inputs like tuples, structs, tuple structs, ... have a single
    /// ident in the signature. The returned patterns contain the correctly named identifiers, so
    /// they can be used as inputs for a function call to the `Callee` function.
    fn to_caller_signature(&self, elem_ident: &Ident, func_ident: &Ident) -> (Signature, Vec<Pat>) {
        let inputs = self
            .0
            .inputs
            .iter()
            .enumerate()
            .map(|(index, fn_arg)| match fn_arg {
                syn::FnArg::Receiver(_) => {
                    abort!(fn_arg, "Methods with `self` are not allowed";
                        help = "Library benchmark functions must be standalone functions, \
                        not methods"
                    )
                }
                syn::FnArg::Typed(pat_type) => {
                    match pattern_to_single_function_ident(&pat_type.pat, elem_ident, index) {
                        Some(pat) => (
                            pat.clone(),
                            FnArg::Typed(PatType {
                                pat: Box::new(pat),
                                ..pat_type.clone()
                            }),
                        ),
                        None => abort!(fn_arg, "Unsupported pattern in function signature";
                            help = "Use simple identifier patterns or destructuring patterns \
                            like tuples, structs, or slices"
                        ),
                    }
                }
            })
            .fold(
                (Vec::new(), Punctuated::new()),
                |(mut vec, mut fn_args), (pat, fn_arg)| {
                    vec.push(pat);
                    fn_args.push(fn_arg);
                    (vec, fn_args)
                },
            );

        (
            Signature {
                ident: func_ident.clone(),
                inputs: inputs.1,
                ..self.0.clone()
            },
            inputs.0,
        )
    }

    fn input_types(&self) -> Vec<Type> {
        self.0
            .inputs
            .iter()
            .map(|fn_arg| match fn_arg {
                syn::FnArg::Receiver(_) => {
                    abort!(fn_arg, "Methods with `self` are not allowed";
                        help = "Library benchmark functions must be standalone functions, \
                        not methods"
                    )
                }
                syn::FnArg::Typed(pat_type) => (*pat_type.ty).clone(),
            })
            .collect()
    }

    fn single_input_type(&self) -> Option<Type> {
        if !self.generics.params.is_empty() {
            return None;
        }

        let mut input_types = self.input_types().into_iter();
        let first = input_types.next()?;
        input_types.next().is_none().then_some(first)
    }
}

impl From<common::Consts> for Consts {
    fn from(value: common::Consts) -> Self {
        Self(value)
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

    fn render_as_code(
        &self,
        setup: &Setup,
        elem_ident: &Ident,
        expected_input_type: Option<&Type>,
    ) -> (TokenStream, TokenStream) {
        let iter_span = self.0.span();
        let iter_ident = Self::iter_ident();

        let iter_count = quote_spanned! { iter_span => #iter_ident.into_iter().count() };
        let iter_expr = self.render_as_expr(setup, Some(&iter_ident));

        // Add the expected benchmark input type if possible for better error messages
        let iter_elem = if let Some(expected_input_type) = expected_input_type {
            quote! {
                let #elem_ident: #expected_input_type = #iter_expr;
            }
        } else {
            quote! {
                let #elem_ident = #iter_expr;
            }
        };

        (iter_count, iter_elem)
    }

    fn render_as_expr(&self, setup: &Setup, iter_ident: Option<&Ident>) -> TokenStream {
        let iter_expr = self.expr();
        let iter_span = self.0.span();
        let index_ident = Self::index_ident();

        let fallback_ident = Self::iter_ident();
        let resolved_ident = iter_ident.unwrap_or(&fallback_ident);

        let prefix = if iter_ident.is_some() {
            quote! {}
        } else {
            quote! { let #resolved_ident = #iter_expr; }
        };

        if let Some(setup) = setup.expr() {
            quote_spanned! { setup.span() =>
                {
                    #prefix
                    #resolved_ident
                        .into_iter()
                        .nth(#index_ident)
                        .map(#setup)
                        .expect("The iterator index should be within bounds")
                }
            }
        } else {
            quote_spanned! { iter_span =>
                {
                    #prefix
                    #resolved_ident
                        .into_iter()
                        .nth(#index_ident)
                        .expect("The iterator index should be within bounds")
                }
            }
        }
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
                        #[rustfmt::skip]
                        abort!(
                            attr, "Only one id is allowed per attribute";
                            help = "Use `#[bench::id]` with a single identifier after `::`";
                            note = r#"#[bench::my_id()] or #[bench::my_id("with", "args")]
    or #[bench::my_id(args = (arg1, ...), config = ...)]"#
                        );
                    }
                    let Some(id) = path_segments.next().map(|p| p.ident.clone()) else {
                        abort!(
                            attr, "An id is required";
                            help = "Use `#[bench::id]` with a unique identifier";
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
                        #[rustfmt::skip]
                        abort!(
                            attr, "Only one id is allowed per attribute";
                            help = "Use `#[benches::id]` with a single identifier after `::`";
                            note = r#"#[benches::my_id("with", "args")]
    or #[benches::my_id(args = [arg1, ...]]"#
                        );
                    }
                    let Some(id) = path_segments.next().map(|p| p.ident.clone()) else {
                        abort!(
                            attr, "An id is required";
                            help = "Use `#[benches::id]` with a unique identifier";
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
                    #[rustfmt::skip]
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
    #[expect(clippy::too_many_lines)]
    fn render_standalone(self, item_fn: &ItemFn) -> TokenStream {
        let new_item_fn = create_item_fn(item_fn);

        let callee = Callee(&item_fn.sig);
        let callee_ident = &callee.ident;

        let elem_ident = format_ident!("__elem");
        let wrapper_ident = format_ident!("wrapper");
        let run_func_ident = format_ident("__run", Some(&wrapper_ident));
        let bench_wrapper_mod = format_ident!("__gungraun_wrapper_mod");
        let shim_mod = format_ident!("__gungraun_wrapper_id_mod");
        let count_ident = format_ident!("__count");
        let has_generics = !item_fn.sig.generics.params.is_empty();

        let config = self.config.render_as_code();

        let input_types = callee.input_types();
        let single_input_type = callee.single_input_type();
        let output_type = match &item_fn.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some(*ty.clone()),
        };
        let shim_func_call = quote!(#wrapper_ident);

        let perf_renderer = PerfRenderer {
            has_generics,
            input_types: &input_types,
            output_type: output_type.as_ref(),
            setup: &self.setup,
            shim_func_call: &shim_func_call,
            shim_mod: &shim_mod,
            single_input_type: single_input_type.as_ref(),
            teardown: &self.teardown,
        };

        let inner = self.setup.render_as_code(&Args::default());
        // Render without "black_box", too so the compiler points to the setup function in error
        // messages.
        let inner_without_black_box = self.setup.render_without_black_box(&Args::default());

        let call_shim_func = if self.setup.is_some() {
            // If possible add the input type for better error messages
            if let Some(input_type) = single_input_type.as_ref() {
                let shim_call =
                    perf_renderer.render_shim_call(&quote!(std::hint::black_box(__setup)));
                self.teardown.render_as_code(quote_spanned! {
                    self.setup.expr().span() => {
                        #[allow(clippy::let_unit_value)]
                        let __setup: #input_type = #inner_without_black_box;
                        std::hint::black_box(
                            #shim_call
                        )
                    }
                })
            } else {
                let shim_call = perf_renderer.render_shim_call(&quote!(__setup));
                self.teardown.render_as_code(quote_spanned! {
                    self.setup.expr().span() => {
                        #[allow(clippy::let_unit_value)]
                        let __setup = #inner;
                        std::hint::black_box(#shim_call)
                    }
                })
            }
        } else {
            let shim_call = perf_renderer.render_shim_call(&inner);
            self.teardown.render_as_code(quote_spanned! {
                inner.span() =>
                    std::hint::black_box(#shim_call)
            })
        };

        let (shim_func, pats) = callee.to_caller_signature(&elem_ident, &wrapper_ident);
        let call_bench_func = quote_spanned! { callee_ident.span() =>
                std::hint::black_box(
                    #bench_wrapper_mod::#callee_ident(#(#pats),*)
                )
        };

        let func = quote! {
            gungraun::__internal::InternalLibFunctionKind::Default(#run_func_ident)
        };

        let run_perf_once = perf_renderer.render_standalone_once(&inner, &inner_without_black_box);

        let run_perf_dynamic =
            perf_renderer.render_standalone_batch(&inner, &inner_without_black_box, None);

        let run_perf_repeat = perf_renderer.render_standalone_batch(
            &inner,
            &inner_without_black_box,
            Some(&count_ident),
        );

        let run_perf_overhead = perf_renderer.render_standalone_overhead_batch(
            &inner,
            &inner_without_black_box,
            &count_ident,
        );

        quote! {
            pub mod #callee_ident {
                use super::*;

                mod __gungraun_wrapper_mod {
                    use super::*;

                    #[inline(never)]
                    #new_item_fn
                }

                pub const __BENCHES: &[gungraun::__internal::InternalMacroLibBench]= &[
                    gungraun::__internal::InternalMacroLibBench {
                        id_display: None,
                        args_display: None,
                        consts_display: None,
                        func: #func,
                        config: None
                    },
                ];

                #config

                mod #shim_mod {
                    use super::*;
                    #[inline(never)]
                    pub(super) #shim_func {
                       #call_bench_func
                    }
                }

                #[inline(never)]
                pub fn #run_func_ident(mode: gungraun::__internal::InternalBenchRunMode) {
                    #[allow(clippy::let_unit_value)]
                    match mode {
                        gungraun::__internal::InternalBenchRunMode::Default => {
                            let _ = #call_shim_func;
                        }
                        gungraun::__internal::InternalBenchRunMode::PerfDynamic => {
                            let _ = #run_perf_dynamic;
                        }
                        gungraun::__internal::InternalBenchRunMode::PerfCalibrate => {
                            gungraun::__internal::perf::calibrate();
                        }
                        gungraun::__internal::InternalBenchRunMode::PerfOverhead(#count_ident) => {
                            let _ = #run_perf_overhead;
                        }
                        gungraun::__internal::InternalBenchRunMode::PerfRepeat(#count_ident) => {
                            let _ = #run_perf_repeat;
                        }
                        gungraun::__internal::InternalBenchRunMode::PerfOnce => {
                            let _ = #run_perf_once;
                        }
                    };
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
    /// `gungraun::__internal::InternalMacroLibBench` structs. This slice can be easily
    /// accessed by the macros of the `gungraun` package in which we finally can call all the
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

                mod __gungraun_wrapper_mod {
                    use super::*;

                    #[inline(never)]
                    #new_item_fn
                }

                pub const __BENCHES: &[gungraun::__internal::InternalMacroLibBench] = &[
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
                        pair, "Invalid parameter: {}", pair.path.require_ident()?;
                        help = "Valid parameters are: `config`, `setup`, `teardown`"
                    );
                }
            }

            let library_benchmark = Self {
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
            quote_spanned! { config.span() =>
                #[inline(never)]
                pub fn #ident()
                    -> Option<gungraun::__internal::InternalLibraryBenchmarkConfig>
                {
                    Some(#config.into())
                }
            }
        } else {
            quote! {
                #[inline(never)]
                pub fn #ident()
                -> Option<gungraun::__internal::InternalLibraryBenchmarkConfig> {
                    None
                }
            }
        }
    }
}

impl PerfRenderer<'_> {
    /// Emits the batched perf body for an `args` benchmark.
    ///
    /// A supplied `setup` creates one owned batch input per repetition; without it, the benchmark
    /// arguments themselves form that input. The shape therefore depends on whether setup is
    /// present and, otherwise, on `args.len()`.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// // Setup present: its output is always one batch input.
    /// let __setup = || -> (Input,) {
    ///     let __setup: Input = setup_expression;
    ///     (std::hint::black_box(__setup),)
    /// };
    /// let __work = |(__input,)| std::hint::black_box(shim(__input));
    ///
    /// // No setup, zero arguments: use Rust's unit value.
    /// let __setup = || {};
    /// let __work = |()| std::hint::black_box(shim());
    ///
    /// // No setup, one argument: preserve the one-element tuple.
    /// let __setup = || (argument,);
    /// let __work = |(__input,)| std::hint::black_box(shim(__input));
    ///
    /// // No setup, multiple arguments: store and destructure an argument tuple.
    /// let __setup = || (arg_a, arg_b);
    /// let __work = |__input| {
    ///     let (arg_a, arg_b) = __input;
    ///     std::hint::black_box(shim(arg_a, arg_b))
    /// };
    /// ```
    ///
    /// This method dispatches to [`Self::render_batch`] to render the remainder.
    fn render_args_batch(
        &self,
        args: &Args,
        inner: &TokenStream,
        inner_without_black_box: &TokenStream,
        count_ident: Option<&Ident>,
    ) -> TokenStream {
        let setup_output = self.setup_output_type();
        let (setup, work) = if self.setup.is_some() {
            let setup_stmt = if let Some(input_type) = self.single_input_type {
                quote_spanned! { self.setup.expr().span() => {
                    let __setup: #input_type = #inner_without_black_box;
                    (std::hint::black_box(__setup),)
                }}
            } else {
                quote_spanned! { self.setup.expr().span() => (#inner,) }
            };

            let shim_call = self.render_shim_call(&quote!(__input));

            (
                setup_stmt,
                quote! { |(__input,)| std::hint::black_box(#shim_call) },
            )
        } else {
            match args.len() {
                0 => {
                    let shim_call = self.render_shim_call(&TokenStream::new());

                    (quote! {}, quote! { |()| std::hint::black_box(#shim_call) })
                }
                1 => {
                    let shim_call = self.render_shim_call(&quote!(__input));

                    (
                        quote_spanned! { args.span() => (#inner,) },
                        quote! { |(__input,)| std::hint::black_box(#shim_call) },
                    )
                }
                len => {
                    let input_idents = (0..len)
                        .map(|index| format_ident!("__input_{index}"))
                        .collect::<Vec<_>>();
                    let shim_call = self.render_shim_call(&quote!(#(#input_idents),*));

                    (
                        quote_spanned! { args.span() => (#inner) },
                        quote! {
                            |__input| {
                                let (#(#input_idents),*) = __input;
                                std::hint::black_box(#shim_call)
                            }
                        },
                    )
                }
            }
        };

        self.render_batch(&setup, setup_output, &work, count_ident)
    }

    /// Emits the one-shot perf body for an `args` benchmark.
    ///
    /// Setup and argument expressions are evaluated before perf is enabled; only the generated
    /// shim call is inside the one-shot timing boundary. The setup and work shape depends on
    /// whether setup is present and, otherwise, on `args.len()`.
    ///
    /// Unlike [`Self::render_args_batch`], no owned input collection or repeated work closure is
    /// needed. For non-generic benchmarks, the generated bindings also receive an explicit tuple
    /// type where available so Rust performs normal argument coercions early which improves and
    /// deduplicates the compiler error messages in case of invalid arguments.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// // Setup present with one input: type-check setup early, then pass its value to the shim.
    /// let __setup: Input = setup_expression;
    /// let __setup = std::hint::black_box(__setup);
    /// let __work = std::hint::black_box(shim(__setup));
    ///
    /// // No setup, zero arguments: call the shim without an input.
    /// let __work = std::hint::black_box(shim());
    ///
    /// // No setup, one argument: bind the argument before timing.
    /// let __arg = argument;
    /// let __work = std::hint::black_box(shim(__arg));
    ///
    /// // No setup, multiple arguments: destructure the tuple before timing.
    /// let (__arg_a, __arg_b) = (arg_a, arg_b);
    /// let __work = std::hint::black_box(shim(__arg_a, __arg_b));
    /// ```
    ///
    /// This method dispatches to [`Self::render_once`] to render the remainder.
    fn render_args_once(
        &self,
        args: &Args,
        inner: &TokenStream,
        inner_without_black_box: &TokenStream,
    ) -> TokenStream {
        let (setup_stmt, work_call) = if self.setup.is_some() {
            let setup_stmt = if let Some(input_type) = self.single_input_type {
                quote_spanned! { self.setup.expr().span() =>
                    #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                    let __setup: #input_type  = #inner_without_black_box;
                    let __setup = std::hint::black_box(__setup);
                }
            } else {
                quote_spanned! { self.setup.expr().span() =>
                    #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                    let __setup = #inner;
                }
            };

            let shim_call = self.render_shim_call(&quote!(__setup));
            (setup_stmt, quote! { std::hint::black_box(#shim_call) })
        } else {
            match args.len() {
                0 => {
                    let shim_call = self.render_shim_call(&TokenStream::new());

                    (quote! {}, quote! { std::hint::black_box(#shim_call) })
                }
                1 => {
                    let setup_stmt = if let Some(input_type) = self.single_input_type {
                        quote_spanned! { args.span() =>
                            #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                            let __arg: #input_type = #inner;
                        }
                    } else {
                        quote_spanned! { args.span() =>
                            #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                            let __arg = #inner;
                        }
                    };

                    let shim_call = self.render_shim_call(&quote!(__arg));
                    (setup_stmt, quote! { std::hint::black_box(#shim_call) })
                }
                len => {
                    let input_idents = (0..len)
                        .map(|index| format_ident!("__arg_{index}"))
                        .collect::<Vec<_>>();

                    let setup_stmt = if self.has_generics {
                        quote_spanned! { args.span() =>
                            #[allow(clippy::let_unit_value)]
                            let (#(#input_idents),*) = (#inner);
                        }
                    } else {
                        let tuple_ty = tuple_type(self.input_types);
                        quote_spanned! { args.span() =>
                            #[allow(clippy::let_unit_value)]
                            let (#(#input_idents),*): #tuple_ty = (#inner);
                        }
                    };
                    let shim_call = self.render_shim_call(&quote!(#(#input_idents),*));

                    (setup_stmt, quote! { std::hint::black_box(#shim_call) })
                }
            }
        };

        self.render_once(&setup_stmt, &work_call)
    }

    /// Emits the setup/work overhead body for an `args` benchmark.
    ///
    /// It prepares inputs before the measurement and measures only their consumption.
    ///
    /// The shared overhead renderer supplies repetition selection, input collection, and the perf
    /// boundary around `__work`; this method supplies only the setup and consumption shapes.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// // Setup present: consume the prepared one-element input.
    /// let __setup = || -> (Input,) {
    ///     let __setup: Input = setup_expression;
    ///     (std::hint::black_box(__setup),)
    /// };
    /// let __work = |(__input,)| { let _ = std::hint::black_box(__input); };
    ///
    /// // No setup, zero arguments: consume unit without an input value.
    /// let __setup = || {};
    /// let __work = |()| { let _ = std::hint::black_box(42); };
    ///
    /// // No setup, one argument: consume the one-element tuple input.
    /// let __setup = || (argument,);
    /// let __work = |(__input,)| { let _ = std::hint::black_box(__input); };
    ///
    /// // No setup, multiple arguments: destructure and consume the argument tuple.
    /// let __setup = || (arg_a, arg_b);
    /// let __work = |__input| {
    ///     let (arg_a, arg_b) = __input;
    ///     let _ = std::hint::black_box((arg_a, arg_b));
    /// };
    /// ```
    ///
    /// Dispatches to [`Self::render_overhead_batch`] to render the remainder.
    fn render_args_overhead_batch(
        &self,
        args: &Args,
        inner: &TokenStream,
        inner_without_black_box: &TokenStream,
        repetitions_ident: &Ident,
    ) -> TokenStream {
        let setup_output = self.setup_output_type();
        let (setup, work) = if self.setup.is_some() {
            let setup_stmt = if let Some(input_type) = self.single_input_type {
                quote_spanned! { self.setup.expr().span() => {
                    let __setup: #input_type = #inner_without_black_box;
                    (std::hint::black_box(__setup),)
                }}
            } else {
                quote_spanned! { self.setup.expr().span() => (#inner,) }
            };

            (
                setup_stmt,
                quote! {
                    |(__input,)| { let _ = std::hint::black_box(__input); }
                },
            )
        } else {
            match args.len() {
                0 => (
                    quote! {},
                    quote! { |()| { let _ = std::hint::black_box(42); } },
                ),
                1 => (
                    quote_spanned! { args.span() => (#inner,) },
                    quote! {
                        |(__input,)| { let _ = std::hint::black_box(__input); }
                    },
                ),
                len => {
                    let input_idents = (0..len)
                        .map(|index| format_ident!("__input_{index}"))
                        .collect::<Vec<_>>();

                    (
                        quote_spanned! { args.span() => (#inner) },
                        quote! {
                            |__input| {
                                let (#(#input_idents),*) = __input;
                                let _ = std::hint::black_box((#(#input_idents),*));
                            }
                        },
                    )
                }
            }
        };

        Self::render_overhead_batch(&setup, setup_output, &work, repetitions_ident)
    }

    /// Emits the batched perf body for an iterator benchmark.
    ///
    /// The selected iterator element becomes a one-item setup tuple and is passed to the shared
    /// batch renderer.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup = || (std::hint::black_box(iter_element),);
    /// let __work = |(__input,)| std::hint::black_box(shim(__input));
    /// ```
    ///
    /// This method dispatches to [`Self::render_batch`] to render the shared batched perf section.
    fn render_iter_batch(&self, iter: &Iter, count_ident: Option<&Ident>) -> TokenStream {
        let setup_output = self.setup_output_type();
        let setup_expr = iter.render_as_expr(self.setup, None);
        let setup_span = self
            .setup
            .expr()
            .map_or_else(|| iter.expr().span(), Spanned::span);

        let setup = if let Some(input_type) = self.single_input_type {
            quote_spanned! { setup_span => {
                let __setup: #input_type = #setup_expr;
                (std::hint::black_box(__setup),)
            }}
        } else {
            quote_spanned! { setup_span => (#setup_expr,) }
        };

        let shim_call = self.render_shim_call(&quote!(__input));
        let work = quote! { |(__input,)| std::hint::black_box(#shim_call) };

        self.render_batch(&setup, setup_output, &work, count_ident)
    }

    /// Emits the one-shot perf body for an iterator benchmark.
    ///
    /// One iterator element is prepared before the single measured shim call.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup: Input = iter_element;
    /// let __setup = std::hint::black_box(__setup);
    /// let __work = std::hint::black_box(shim(__setup));
    /// ```
    ///
    /// This method dispatches to [`Self::render_once`] to render the shared one-shot perf section.
    fn render_iter_once(&self, iter: &Iter) -> TokenStream {
        let setup_expr = iter.render_as_expr(self.setup, None);
        let setup_span = self
            .setup
            .expr()
            .map_or_else(|| iter.expr().span(), Spanned::span);

        let setup = if let Some(input_type) = self.single_input_type {
            quote_spanned! { setup_span => {
                let __setup: #input_type = #setup_expr;
                std::hint::black_box(__setup)
            }}
        } else {
            quote_spanned! { setup_span =>
                std::hint::black_box(#setup_expr)
            }
        };

        let setup = quote_spanned! { setup_span =>
            #[allow(clippy::let_unit_value, clippy::useless_conversion)]
            let __setup = #setup;
        };
        let shim_call = self.render_shim_call(&quote!(__setup));
        let work = quote! { std::hint::black_box(#shim_call) };

        self.render_once(&setup, &work)
    }

    /// Emits the setup/work overhead body for an iterator benchmark.
    ///
    /// It prepares iterator elements and measures only their `black_box` consumption.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup = || -> (Input,) {
    ///     (std::hint::black_box(iter_element),)
    /// };
    /// let __work = |(__input,)| { let _ = std::hint::black_box(__input); };
    /// ```
    ///
    /// This method dispatches to [`Self::render_overhead_batch`] to render the remainder.
    fn render_iter_overhead_batch(&self, iter: &Iter, repetitions_ident: &Ident) -> TokenStream {
        let setup_output = self.setup_output_type();
        let setup_expr = iter.render_as_expr(self.setup, None);
        let setup_span = self
            .setup
            .expr()
            .map_or_else(|| iter.expr().span(), Spanned::span);

        let setup = if let Some(input_type) = self.single_input_type {
            quote_spanned! { setup_span => {
                let __setup: #input_type = #setup_expr;
                (std::hint::black_box(__setup),)
            }}
        } else {
            quote_spanned! { setup_span => (#setup_expr,) }
        };

        let work = quote! {
            |(__input,)| { let _ = std::hint::black_box(__input); }
        };

        Self::render_overhead_batch(&setup, setup_output, &work, repetitions_ident)
    }

    /// Helper method which emits the generated wrapper-module call used by benchmark bodies.
    ///
    /// ```rust,ignore
    /// shim_mod::shim_func(arguments)
    /// ```
    fn render_shim_call(&self, arguments: &TokenStream) -> TokenStream {
        let shim_func_call = self.shim_func_call;
        let shim_mod = self.shim_mod;

        quote!(#shim_mod::#shim_func_call(#arguments))
    }

    /// Emits the batched perf body for a standalone library benchmark.
    ///
    /// Standalone setup produces the typed input, then the shared batch renderer supplies
    /// repetition selection and the perf timing boundary.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup = || (standalone_setup(),);
    /// let __work = |(__input,)| std::hint::black_box(shim(__input));
    /// ```
    ///
    /// This method dispatches to [`Self::render_batch`] to render the remainder.
    fn render_standalone_batch(
        &self,
        inner: &TokenStream,
        inner_without_black_box: &TokenStream,
        count_ident: Option<&Ident>,
    ) -> TokenStream {
        let setup_output = Some(tuple_type(self.input_types));
        let (setup, work) = if self.setup.is_some() {
            let setup_stmt = if let Some(input_type) = self.single_input_type {
                quote_spanned! { self.setup.expr().span() => {
                    let __setup: #input_type = #inner_without_black_box;
                    let __setup = (std::hint::black_box(__setup),);
                    __setup
                }}
            } else {
                quote_spanned! { self.setup.expr().span() => (#inner,) }
            };

            let shim_call = self.render_shim_call(&quote!(__input));

            (
                setup_stmt,
                quote! { |(__input,)| std::hint::black_box(#shim_call) },
            )
        } else {
            let shim_call = self.render_shim_call(&TokenStream::new());

            (quote! {}, quote! { |()| std::hint::black_box(#shim_call) })
        };

        self.render_batch(&setup, setup_output, &work, count_ident)
    }

    /// Emits the one-shot perf body for a standalone library benchmark.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup: Input = standalone_setup();
    /// let __setup = std::hint::black_box(__setup);
    /// let __work = std::hint::black_box(shim(__setup));
    /// ```
    ///
    /// This method dispatches to [`Self::render_once`] to render the remainder.
    fn render_standalone_once(
        &self,
        inner: &TokenStream,
        inner_without_black_box: &TokenStream,
    ) -> TokenStream {
        let (setup_stmt, work_call) = if self.setup.is_some() {
            let setup_stmt = if let Some(input_type) = self.single_input_type {
                quote_spanned! { self.setup.expr().span() =>
                    #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                    let __setup: #input_type = #inner_without_black_box;
                    let __setup = std::hint::black_box(__setup);
                }
            } else {
                quote_spanned! { self.setup.expr().span() =>
                    #[allow(clippy::let_unit_value, clippy::useless_conversion)]
                    let __setup = #inner;
                }
            };

            let shim_call = self.render_shim_call(&quote!(__setup));

            (setup_stmt, quote! { std::hint::black_box(#shim_call) })
        } else {
            let shim_call = self.render_shim_call(&TokenStream::new());

            (quote! {}, quote! { std::hint::black_box(#shim_call) })
        };

        self.render_once(&setup_stmt, &work_call)
    }

    /// Emits the setup/work overhead body for a standalone library benchmark.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup = || -> (Input,) {
    ///     (std::hint::black_box(standalone_setup()),)
    /// };
    /// let __work = |(__input,)| { let _ = std::hint::black_box(__input); };
    /// ```
    ///
    /// This method dispatches to [`Self::render_overhead_batch`] to render the remainder.
    fn render_standalone_overhead_batch(
        &self,
        inner: &TokenStream,
        inner_without_black_box: &TokenStream,
        repetitions_ident: &Ident,
    ) -> TokenStream {
        let setup_output = Some(tuple_type(self.input_types));
        let (setup, work) = if self.setup.is_some() {
            let setup_stmt = if let Some(input_type) = self.single_input_type {
                quote_spanned! { self.setup.expr().span() => {
                    let __setup: #input_type = #inner_without_black_box;
                    let __setup = (std::hint::black_box(__setup),);
                    __setup
                }}
            } else {
                quote_spanned! { self.setup.expr().span() => (#inner,) }
            };

            (
                setup_stmt,
                quote! {
                    |(__input,)| { let _ = std::hint::black_box(__input); }
                },
            )
        } else {
            (
                quote! {},
                quote! { |()| { let _ = std::hint::black_box(42); } },
            )
        };

        Self::render_overhead_batch(&setup, setup_output, &work, repetitions_ident)
    }

    /// Helper method which emits code that chooses the number of perf repetitions for this
    /// benchmark.
    ///
    /// An omitted `perf` attribute is represented as [`None`] and defaults to dynamic calibration.
    /// A fixed value emits that count directly.
    ///
    /// Emits code that selects the number of repetitions used by a perf batch.
    ///
    ///
    ///
    /// ```rust,ignore
    /// let __repetitions = supplied_count_or_calibrated_count;
    /// ```
    fn render_perf_repetitions(repetitions: common::PerfRepetition) -> TokenStream {
        match repetitions {
            common::PerfRepetition::Dynamic => quote! {
                let __repetitions = gungraun::__internal::stats::calibrate_linear(
                    std::time::Duration::from_millis(50),
                    &__setup,
                    &__work,
                    &__teardown,
                );
            },
            common::PerfRepetition::Fixed(ident) => quote! {
                let __repetitions = #ident;
            },
        }
    }

    /// Emits the common batched perf measurement body.
    ///
    /// The generated code creates all setup inputs before enabling perf, runs the work closure for
    /// the full batch while perf is enabled, and runs teardown after perf is disabled. When
    /// present, `setup_output` annotates the setup closure return type so Rust performs normal
    /// argument coercions before calibration unifies the setup output with the work input.
    ///
    /// Emits the shared batched perf section used by args, iterator, and standalone renderers.
    ///
    /// Setup and input collection happen before perf is enabled; only the work closure is timed.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup = || -> SetupOutput { setup_expression };
    /// let __work = |__input| work_expression(__input);
    /// let __teardown = |__result| teardown_expression(__result);
    ///
    /// let __repetitions = /* selected repetition count */;
    /// gungraun::perf_log!(
    ///     "{} {}", gungraun::__internal::PERF_REPETITIONS_MARKER, __repetitions
    /// );
    ///
    /// let __inputs = (0..__repetitions).map(|_| __setup()).collect::<Vec<_>>();
    ///
    /// let __lock = gungraun::perf_enable!();
    /// let __results = __inputs.into_iter().map(__work).collect::<Vec<_>>();
    /// gungraun::perf_disable!(__lock);
    ///
    /// for __result in __results {
    ///     __teardown(__result);
    /// }
    /// ```
    fn render_batch(
        &self,
        setup: &TokenStream,
        setup_output: Option<TokenStream>,
        work: &TokenStream,
        count_ident: Option<&Ident>,
    ) -> TokenStream {
        let setup = if let Some(setup_output) = setup_output {
            quote! {
                #[allow(clippy::unused_unit, clippy::useless_conversion)]
                let __setup = || -> #setup_output { #setup };
            }
        } else {
            quote! {
                #[allow(clippy::unused_unit, clippy::useless_conversion)]
                let __setup = || { #setup };
            }
        };

        let repetitions = if let Some(count_ident) = count_ident {
            Self::render_perf_repetitions(common::PerfRepetition::Fixed(count_ident.clone()))
        } else {
            Self::render_perf_repetitions(common::PerfRepetition::Dynamic)
        };

        let teardown = {
            if let Some(teardown) = self.teardown.0.0.as_ref() {
                let type_annotation = if self.has_generics {
                    quote! {}
                } else if let Some(ty) = self.output_type {
                    quote_spanned! { teardown.span() => : #ty }
                } else {
                    quote_spanned! { teardown.span() => : () }
                };

                quote_spanned! { teardown.span() => |__result #type_annotation| {
                        let _ = std::hint::black_box(#teardown(__result));
                    }
                }
            } else {
                quote! { |__result| { let _ = __result; } }
            }
        };

        quote! {
            {
                #setup
                let __work = #work;
                let __teardown = #teardown;

                #repetitions

                gungraun::perf_log!(
                    "{} {}", gungraun::__internal::PERF_REPETITIONS_MARKER, __repetitions
                );

                let __inputs = (0..__repetitions).map(|_| __setup()).collect::<Vec<_>>();

                let __lock = gungraun::perf_enable!();
                #[allow(clippy::useless_conversion)]
                let __results = __inputs.into_iter().map(__work).collect::<Vec<_>>();
                gungraun::perf_disable!(__lock);

                for __result in __results {
                    __teardown(__result);
                }
            }
        }
    }

    /// Emits the shared batched overhead section used to measure setup and collection costs.
    ///
    /// Schematic generated code:
    ///
    /// ```rust,ignore
    /// let __setup = || -> SetupOutput { setup_expression };
    /// let __work = |__input| work_expression(__input);
    ///
    /// let __repetitions = repetitions;
    /// gungraun::perf_log!(
    ///     "{} {}", gungraun::__internal::PERF_REPETITIONS_MARKER, __repetitions
    /// );
    ///
    /// let __inputs = (0..__repetitions).map(|_| __setup()).collect::<Vec<_>>();
    ///
    /// let __lock = gungraun::perf_enable!();
    /// let __results = __inputs.into_iter().map(__work).collect::<Vec<_>>();
    /// gungraun::perf_disable!(__lock);
    ///
    /// let _ = __results;
    /// ```
    fn render_overhead_batch(
        setup: &TokenStream,
        setup_output: Option<TokenStream>,
        work: &TokenStream,
        repetitions_ident: &Ident,
    ) -> TokenStream {
        let setup = if let Some(setup_output) = setup_output {
            quote! {
                #[allow(clippy::unused_unit, clippy::useless_conversion)]
                let __setup = || -> #setup_output { #setup };
            }
        } else {
            quote! {
                #[allow(clippy::unused_unit, clippy::useless_conversion)]
                let __setup = || { #setup };
            }
        };

        quote! {
            {
                #setup
                let __work = #work;
                let __repetitions = #repetitions_ident;

                gungraun::perf_log!(
                    "{} {}", gungraun::__internal::PERF_REPETITIONS_MARKER, __repetitions
                );

                let __inputs = (0..__repetitions).map(|_| __setup()).collect::<Vec<_>>();

                let __lock = gungraun::perf_enable!();
                #[allow(clippy::useless_conversion)]
                let __results = __inputs.into_iter().map(__work).collect::<Vec<_>>();
                gungraun::perf_disable!(__lock);

                let _ = __results;
            }
        }
    }

    /// Emits the shared one-shot perf section used by args, iterator, and standalone renderers.
    ///
    /// ```rust,ignore
    /// #setup
    ///
    /// let __lock = gungraun::perf_enable!();
    /// let __result = #work;
    /// gungraun::perf_disable!(__lock);
    ///
    /// #after_perf
    /// ```
    fn render_once(&self, setup: &TokenStream, work: &TokenStream) -> TokenStream {
        let after_perf = if let Some(teardown) = self.teardown.0.0.as_ref() {
            quote_spanned! { teardown.span() =>
                let _ = std::hint::black_box(#teardown(__result));
            }
        } else {
            quote! { let _ = __result; }
        };

        quote! {
            {
                #setup

                let __lock = gungraun::perf_enable!();
                let __result = #work;
                gungraun::perf_disable!(__lock);

                #after_perf
            }
        }
    }

    fn setup_output_type(&self) -> Option<TokenStream> {
        (!self.has_generics).then(|| tuple_type(self.input_types))
    }
}

impl Setup {
    fn is_some(&self) -> bool {
        self.0.0.is_some()
    }

    fn expr(&self) -> Option<&ExprPath> {
        self.0.0.as_ref()
    }

    fn render_as_code(&self, args: &Args) -> TokenStream {
        if let Some(setup) = &self.deref().0 {
            quote_spanned! { setup.span() => std::hint::black_box(#setup(#args)) }
        } else {
            quote_spanned! { args.span() => #args }
        }
    }

    fn render_without_black_box(&self, args: &Args) -> TokenStream {
        if let Some(setup) = &self.deref().0 {
            quote_spanned! { setup.span() => #setup(#args) }
        } else {
            quote_spanned! { args.span() => #args }
        }
    }
}

impl Teardown {
    fn render_as_code(&self, tokens: TokenStream) -> TokenStream {
        if let Some(teardown) = &self.deref().0 {
            quote_spanned! { teardown.span() => {
                    #[allow(clippy::let_unit_value)]
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
    let vis = parse_quote_spanned! { item_fn.span() => pub(super) };
    let item_fn_block = item_fn.block.clone();
    let block = parse_quote_spanned!( item_fn_block.span() =>
        {
            gungraun::client_requests::cachegrind::start_instrumentation();
            #[allow(clippy::let_unit_value)]
            let __r = #item_fn_block;
            gungraun::client_requests::cachegrind::stop_instrumentation();
            __r
        }
    );
    ItemFn {
        attrs: vec![],
        vis,
        sig: item_fn.sig.clone(),
        block,
        modifiers: item_fn.modifiers.clone(),
    }
}

#[cfg(not(feature = "cachegrind"))]
fn create_item_fn(item_fn: &ItemFn) -> ItemFn {
    let vis = parse_quote_spanned! { item_fn.span() => pub(super) };
    ItemFn {
        attrs: vec![],
        vis,
        sig: item_fn.sig.clone(),
        block: item_fn.block.clone(),
        modifiers: item_fn.modifiers.clone(),
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

fn tuple_type(types: &[Type]) -> TokenStream {
    match types {
        [] => quote! { () },
        [ty] => quote! { (#ty,) },
        types => quote! { (#(#types),*) },
    }
}
