use std::ops::Deref;

use derive_more::{Deref as DerefDerive, DerefMut as DerefMutDerive};
use proc_macro2::TokenStream;
use proc_macro_error2::abort;
use quote::{format_ident, quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse2, parse_quote, parse_quote_spanned, Attribute, Expr, ExprPath, FnArg, Generics, Ident,
    ItemFn, MetaNameValue, Pat, PatType, Signature, Token,
};

use crate::common::{
    self, format_ident, pattern_to_single_function_ident, truncate_str_utf8, BenchesArgs,
    BenchesConsts, File,
};
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
    config: BenchConfig,
    consts: Consts,
    generics: Generics,
    id: Ident,
    mode: BenchMode,
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

        if let Ok(pairs) =
            meta.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
        {
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
                        pair, "Invalid argument: {}", pair.path.require_ident()?;
                        help = "Valid arguments are: `args`, `consts`, `config`, \
                        `setup`, `teardown`"
                    );
                }
            }
        } else {
            args.parse_meta_list(meta)?;
        }

        setup.update(other_setup);
        teardown.update(other_teardown);

        args.check_num_arguments(expected_num_args, setup.is_some());
        consts.check_num_arguments(expected_num_consts);

        Ok(Self {
            id,
            mode: BenchMode::Args(args),
            config,
            setup,
            teardown,
            consts,
            generics,
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

        if let Ok(pairs) =
            meta.parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
        {
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
                        pair, "Invalid argument: {}", pair.path.require_ident()?;
                        help = "Valid arguments are: `args`, `consts`, `file`, `iter`, `config`, \
                        `setup`, `teardown`"
                    );
                }
            }
        } else {
            args = BenchesArgs::from_meta_list(meta, expected_num_args)?;
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
        .map(|b| Self {
            id: b.id,
            mode: b.mode.into(),
            config: config.clone(),
            setup: setup.clone(),
            teardown: teardown.clone(),
            consts: b.consts.map_or_else(Consts::default, Into::into),
            generics: generics.clone(),
        })
        .collect();

        Ok(benches)
    }

    fn render_as_code(&self, callee: &Callee) -> TokenStream {
        let bench_id = &self.id;
        let elem_ident = format_ident!("__elem");
        let run_func_id = format_ident("__run", Some(bench_id));
        let callee_ident = &callee.ident;

        let func = match &self.mode {
            // The amount of input arguments of the benchmark function is already verified to be
            // exactly one
            BenchMode::Iter(iter) => {
                let iter_expr = iter.expr();

                let index_ident = Iter::index_ident();
                let iter_ident = Iter::iter_ident();

                let (iter_count, iter_elem) = iter.render_as_code(&self.setup);

                let (bench_func_call, bench_id_call) =
                    self.consts
                        .to_function_calls(&self.generics, callee_ident, Some(bench_id));

                let (bench_id_sig, pats) = callee.to_caller_signature(&elem_ident, bench_id);
                let bench_id_mod = format_ident("__gungraun_wrapper_id_mod", Some(bench_id));
                let call_bench_func = quote_spanned! { callee_ident.span() =>
                    std::hint::black_box(
                        __gungraun_wrapper_mod::#bench_func_call(#(#pats),*)
                    )
                };

                let call_bench_id = self.teardown.render_as_code(quote_spanned! {
                    bench_id.span() => #bench_id_mod::#bench_id_call(#elem_ident)
                });

                quote!(
                    mod #bench_id_mod {
                        use super::*;
                        #[inline(never)]
                        pub(super) #bench_id_sig {
                           #call_bench_func
                        }
                    }

                    #[inline(never)]
                    pub fn #run_func_id(#index_ident: Option<usize>) -> usize {
                       let #iter_ident = #iter_expr;

                       if let Some(#index_ident) = #index_ident {
                           #[allow(clippy::useless_conversion)]
                           let #elem_ident = #iter_elem;
                           #[allow(clippy::let_unit_value)]
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
                let bench_id_mod = format_ident("__gungraun_wrapper_id_mod", Some(bench_id));

                let (bench_func_call, bench_id_call) =
                    self.consts
                        .to_function_calls(&self.generics, callee_ident, Some(bench_id));

                // There is a difference to allow the clippy let_unit_value lint
                let call_bench_id = if self.setup.is_some() {
                    self.teardown.render_as_code(quote_spanned! {
                        bench_id.span() => {
                            #[allow(clippy::let_unit_value)]
                            let __setup = #inner;
                            std::hint::black_box(#bench_id_mod::#bench_id_call(__setup))
                        }
                    })
                } else {
                    self.teardown
                        .render_as_code(quote_spanned! { bench_id.span() =>
                            std::hint::black_box(#bench_id_mod::#bench_id_call(#inner))
                        })
                };

                let (bench_id_func, pats) = callee.to_caller_signature(&elem_ident, bench_id);
                let call_bench_func = quote_spanned! { callee_ident.span() =>
                        std::hint::black_box(
                            __gungraun_wrapper_mod::#bench_func_call(#(#pats),*)
                        )
                };

                quote!(
                    mod #bench_id_mod {
                        use super::*;
                        #[inline(never)]
                        pub(super) #bench_id_func {
                           #call_bench_func
                        }
                    }
                    #[inline(never)]
                    pub fn #run_func_id() {
                       #[allow(clippy::let_unit_value)]
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
    #[allow(unused)]
    fn len_inputs(&self) -> usize {
        self.0.inputs.len()
    }

    /// Convert to the function signature of the function calling the `Callee` (benchmark function)
    ///
    /// All elements with multiple inputs like tuples, structs, tuple structs, ... have a single
    /// ident in the signature. The returned patterns contain the correctly named identifiers, so
    /// they can be used as inputs for a function call to the `Callee` function.
    fn to_caller_signature(&self, elem_ident: &Ident, bench_id: &Ident) -> (Signature, Vec<Pat>) {
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
                ident: bench_id.clone(),
                inputs: inputs.1,
                ..self.0.clone()
            },
            inputs.0,
        )
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
                self.setup.expr().span() => {
                    #[allow(clippy::let_unit_value)]
                    let __setup = #inner;
                    std::hint::black_box(__gungraun_wrapper_id_mod::#wrapper_ident(__setup))
                }
            })
        } else {
            self.teardown.render_as_code(quote_spanned! {
                inner.span() =>
                    std::hint::black_box(__gungraun_wrapper_id_mod::#wrapper_ident(#inner))
            })
        };

        let (wrapper_func, pats) = callee.to_caller_signature(&elem_ident, &wrapper_ident);
        let call_bench_func = quote_spanned! { callee_ident.span() =>
                std::hint::black_box(
                    __gungraun_wrapper_mod::#callee_ident(#(#pats),*)
                )
        };

        let func = quote! {
            gungraun::__internal::InternalLibFunctionKind::Default(#run_func_id)
        };

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

                mod __gungraun_wrapper_id_mod {
                    use super::*;
                    #[inline(never)]
                    pub(super) #wrapper_func {
                       #call_bench_func
                    }
                }

                #[inline(never)]
                pub fn #run_func_id() {
                   #[allow(clippy::let_unit_value)]
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
                        pair, "Invalid argument: {}", pair.path.require_ident()?;
                        help = "Valid arguments are: `config`, `setup`, `teardown`"
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
