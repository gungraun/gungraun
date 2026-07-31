use std::fmt::Display;
use std::ops::Deref;

use derive_more::{Deref as DerefDerive, DerefMut as DerefMutDerive};
use proc_macro_error3::abort;
use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, format_ident, quote};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Generics, Ident, ItemFn, MetaNameValue, Token, parse_quote, parse2};

use crate::common::{self, BenchesArgs, BenchesConsts, File, format_ident, truncate_str_utf8};
use crate::{CargoMetadata, defaults};

#[derive(Debug)]
enum BenchMode {
    Iter(Iter),
    Args(Args),
}

/// This struct reflects the `args` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct Args(common::Args);

#[derive(Debug)]
struct AssistantRenderer;

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

/// This is the counterpart to the `#[binary_benchmark]` attribute.
#[derive(Debug, Default)]
struct BinaryBenchmark {
    benches: Vec<Bench>,
    config: BinaryBenchmarkConfig,
    setup: Setup,
    teardown: Teardown,
}

/// The `config` parameter of the `#[binary_benchmark]` attribute
///
/// The `BenchConfig` and `BinaryBenchmarkConfig` are rendered differently, hence the different
/// structures
///
/// Note: This struct is completely independent of the `gungraun::BinaryBenchmarkConfig`
/// struct with the same name.
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct BinaryBenchmarkConfig(common::BenchConfig);

/// This struct reflects the `consts` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone, DerefDerive, DerefMutDerive)]
struct Consts(common::Consts);

#[derive(Debug, Clone)]
struct Iter(Expr);

#[derive(Debug, Default, Clone)]
struct Setup(Option<Expr>);

#[derive(Debug, Default, Clone)]
struct Teardown(Option<Expr>);

impl ToTokens for Args {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.deref().to_tokens(tokens);
    }
}

impl Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tokens = self.to_tokens_without_black_box().to_string();
        write!(f, "{tokens}")
    }
}

impl AssistantRenderer {
    fn render_as_code(assistant_id: &Ident, expr: Option<&Expr>, args: &Args) -> TokenStream {
        if let Some(setup) = expr {
            let call = if let Expr::Path(path) = &setup {
                quote!(#path(#args))
            } else {
                quote!(#setup)
            };
            quote! {
                pub fn #assistant_id() {
                    #call;
                }
            }
        } else {
            TokenStream::new()
        }
    }

    fn render_as_iter_code(assistant_id: &Ident, expr: Option<&Expr>, iter: &Iter) -> TokenStream {
        match &expr {
            Some(Expr::Path(path)) => {
                let iter_expr = &iter.0;
                let iter_index = format_ident!("__iter_index");
                let iter_ident = format_ident!("__iter");
                let elem_ident = format_ident!("__elem");
                quote! {
                    pub fn #assistant_id(#iter_index: Option<usize>) {
                        let #iter_ident = #iter_expr;

                        let #elem_ident = #iter_ident
                            .into_iter()
                            .nth(#iter_index.expect("The iterator index should be present"))
                            .expect("The iterator index should be within bounds");

                        #path(#elem_ident);
                    }
                }
            }
            Some(expr) => quote! {
                pub fn #assistant_id() {
                    #expr;
                }
            },
            None => TokenStream::new(),
        }
    }

    fn render_as_member(
        assistant_id: &Ident,
        expr: Option<&Expr>,
        iter: Option<&Iter>,
    ) -> TokenStream {
        match expr {
            Some(Expr::Path(_)) if iter.is_some() => quote! {
                gungraun::__internal::InternalBinAssistantKind::Iter(#assistant_id)
            },
            Some(_) => quote! {
                gungraun::__internal::InternalBinAssistantKind::Default(#assistant_id)
            },
            None => quote! { gungraun::__internal::InternalBinAssistantKind::None },
        }
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
                            help = "Valid parameters are: `args`, `consts`, `config`, `setup`, \
                            `teardown`"
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

        Ok(Self {
            id,
            consts,
            generics,
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
                            help = "Valid parameters are: `args`, `consts`, `file`, `iter`, `config`, \
                            `setup`, `teardown`"
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

    fn render_as_code(&self, callee: &Ident) -> TokenStream {
        let id = &self.id;
        let (bench_func_call, _) = self.consts.to_function_calls(&self.generics, callee, None);
        match &self.mode {
            BenchMode::Iter(iter) => {
                let iter_expr = &iter.0;

                let func = quote!(
                    pub fn #id() -> Vec<gungraun::Command> {
                        let __iter = #iter_expr;

                        #[allow(clippy::useless_conversion)]
                        __iter.into_iter().map(|__elem| #bench_func_call(__elem)).collect()
                    }
                );

                let config = self.config.render_as_code(Some(id));
                let setup = self.setup.render_as_iter_code(Some(id), iter);
                let teardown = self.teardown.render_as_iter_code(Some(id), iter);

                quote! {
                    #config
                    #setup
                    #teardown
                    #func
                }
            }
            BenchMode::Args(args) => {
                let args_tokens = args.to_tokens_without_black_box();
                let func = quote!(
                    pub fn #id() -> gungraun::Command {
                        #bench_func_call(#args_tokens)
                    }
                );

                let config = self.config.render_as_code(Some(id));
                let setup = self.setup.render_as_code(Some(id), args);
                let teardown = self.teardown.render_as_code(Some(id), args);

                quote! {
                    #config
                    #setup
                    #teardown
                    #func
                }
            }
        }
    }

    fn render_as_member(&self) -> TokenStream {
        let id = &self.id;
        let id_display = self.id.to_string();
        let config = self.config.render_as_member(Some(id));

        let (args_string, func_kind, setup, teardown) = match &self.mode {
            BenchMode::Iter(iter) => (
                self.setup.to_string_with_iter(iter),
                quote! {Iter(#id)},
                self.setup.render_as_member(Some(id), Some(iter)),
                self.teardown.render_as_member(Some(id), Some(iter)),
            ),
            BenchMode::Args(args) => (
                self.setup.to_string_with_args(args),
                quote! {Default(#id)},
                self.setup.render_as_member(Some(id), None),
                self.teardown.render_as_member(Some(id), None),
            ),
        };

        let func = quote!(gungraun::__internal::InternalBinFunctionKind::#func_kind);

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
            gungraun::__internal::InternalMacroBinBench {
                id_display: Some(#id_display),
                args_display: #args_display,
                consts_display: #consts_display,
                func: #func,
                config: #config,
                setup: #setup,
                teardown: #teardown,
            }
        }
    }
}

impl BenchConfig {
    pub fn ident(id: Option<&Ident>) -> Ident {
        format_ident("__get_config", id)
    }

    fn render_as_code(&self, id: Option<&Ident>) -> TokenStream {
        if let Some(config) = &self.deref().0 {
            let ident = Self::ident(id);
            quote! {
                pub fn #ident() -> gungraun::__internal::InternalBinaryBenchmarkConfig {
                    #config.into()
                }
            }
        } else {
            TokenStream::new()
        }
    }

    fn render_as_member(&self, id: Option<&Ident>) -> TokenStream {
        if self.deref().is_some() {
            let ident = Self::ident(id);
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

impl BinaryBenchmark {
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

    /// Render the `#[binary_benchmark]` attribute when no outer attribute was present
    ///
    /// ```ignore
    /// #[binary_benchmark]
    /// fn my_benchmark_function() -> u64 {
    ///     my_lib::bench_me(42)
    /// }
    /// ```
    fn render_standalone(self, item_fn: &ItemFn) -> TokenStream {
        let ident = &item_fn.sig.ident;
        let visibility: syn::Visibility = parse_quote! { pub };
        let new_item_fn = ItemFn {
            attrs: vec![],
            vis: visibility,
            sig: item_fn.sig.clone(),
            block: item_fn.block.clone(),
            modifiers: item_fn.modifiers.clone(),
        };

        let config = self.config.render_as_code();
        let setup = self.setup.render_as_code(None, &Args::default());
        let setup_member = self.setup.render_as_member(None, None);
        let teardown = self.teardown.render_as_code(None, &Args::default());
        let teardown_member = self.teardown.render_as_member(None, None);

        quote! {
            pub mod #ident {
                use super::*;

                #new_item_fn

                pub const __BENCHES: &[gungraun::__internal::InternalMacroBinBench]= &[
                    gungraun::__internal::InternalMacroBinBench {
                        id_display: None,
                        args_display: None,
                        consts_display: None,
                        func: gungraun::__internal::InternalBinFunctionKind::Default(#ident),
                        setup: #setup_member,
                        teardown: #teardown_member,
                        config: None
                    },
                ];

                #config
                #setup
                #teardown
            }
        }
    }

    fn render_benches(self, item_fn: &ItemFn) -> TokenStream {
        let new_item_fn = ItemFn {
            attrs: vec![],
            vis: syn::Visibility::Inherited,
            sig: item_fn.sig.clone(),
            block: item_fn.block.clone(),
            modifiers: item_fn.modifiers.clone(),
        };

        let mod_name = &item_fn.sig.ident;
        let callee = &item_fn.sig.ident;
        let mut funcs = TokenStream::new();
        let mut bin_benches = vec![];
        for bench in self.benches {
            funcs.append_all(bench.render_as_code(callee));
            bin_benches.push(bench.render_as_member());
        }

        let config = self.config.render_as_code();
        quote! {
            pub mod #mod_name {
                use super::*;

                #new_item_fn

                pub const __BENCHES: &[gungraun::__internal::InternalMacroBinBench] = &[
                    #(#bin_benches,)*
                ];

                #config

                #funcs
            }
        }
    }
}

impl Parse for BinaryBenchmark {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(Self::default())
        } else {
            let mut config = BinaryBenchmarkConfig::default();
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

            let binary_benchmark = Self {
                config,
                setup,
                teardown,
                benches: vec![],
            };
            Ok(binary_benchmark)
        }
    }
}

impl BinaryBenchmarkConfig {
    fn render_as_code(&self) -> TokenStream {
        if let Some(config) = &self.deref().0 {
            quote!(
                pub fn __get_config()
                    -> Option<gungraun::__internal::InternalBinaryBenchmarkConfig>
                {
                    Some(#config.into())
                }
            )
        } else {
            quote!(
                pub fn __get_config() -> Option<gungraun::__internal::InternalBinaryBenchmarkConfig>
                {
                    None
                }
            )
        }
    }
}

impl From<common::Consts> for Consts {
    fn from(value: common::Consts) -> Self {
        Self(value)
    }
}

impl Display for Iter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_token_stream())
    }
}

impl Setup {
    pub fn ident(id: Option<&Ident>) -> Ident {
        format_ident("__setup", id)
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) {
        if self.0.is_none() {
            self.0 = Some(pair.value.clone());
        } else {
            abort!(
                pair, "Duplicate parameter: `setup`";
                help = "`setup` is allowed only once"
            );
        }
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// If this Setup is none and the other setup has a value update this `Setup` with that value
    pub fn update(&mut self, other: &Self) {
        if let (None, Some(other)) = (&self.0, &other.0) {
            self.0 = Some(other.clone());
        }
    }

    pub fn to_string_with_args(&self, args: &Args) -> String {
        match &self.0 {
            Some(Expr::Path(setup)) => {
                format!("{}({args})", setup.to_token_stream())
            }
            Some(_) | None => args.to_string(),
        }
    }

    pub fn to_string_with_iter(&self, iter: &Iter) -> String {
        match &self.0 {
            Some(Expr::Path(setup)) => {
                format!("{}(nth of {iter})", setup.to_token_stream())
            }
            Some(_) | None => {
                format!("nth of {iter}")
            }
        }
    }

    fn render_as_code(&self, id: Option<&Ident>, args: &Args) -> TokenStream {
        AssistantRenderer::render_as_code(&Self::ident(id), self.0.as_ref(), args)
    }

    fn render_as_iter_code(&self, id: Option<&Ident>, iter: &Iter) -> TokenStream {
        AssistantRenderer::render_as_iter_code(&Self::ident(id), self.0.as_ref(), iter)
    }

    fn render_as_member(&self, id: Option<&Ident>, iter: Option<&Iter>) -> TokenStream {
        AssistantRenderer::render_as_member(&Self::ident(id), self.0.as_ref(), iter)
    }
}

impl Teardown {
    pub fn ident(id: Option<&Ident>) -> Ident {
        format_ident("__teardown", id)
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) {
        if self.0.is_none() {
            self.0 = Some(pair.value.clone());
        } else {
            abort!(
                pair, "Duplicate parameter: `teardown`";
                help = "`teardown` is allowed only once"
            );
        }
    }

    /// If this Setup is none and the other setup has a value update this `Setup` with that value
    pub fn update(&mut self, other: &Self) {
        if let (None, Some(other)) = (&self.0, &other.0) {
            self.0 = Some(other.clone());
        }
    }

    fn render_as_code(&self, id: Option<&Ident>, args: &Args) -> TokenStream {
        AssistantRenderer::render_as_code(&Self::ident(id), self.0.as_ref(), args)
    }

    fn render_as_iter_code(&self, id: Option<&Ident>, iter: &Iter) -> TokenStream {
        AssistantRenderer::render_as_iter_code(&Self::ident(id), self.0.as_ref(), iter)
    }

    fn render_as_member(&self, id: Option<&Ident>, iter: Option<&Iter>) -> TokenStream {
        AssistantRenderer::render_as_member(&Self::ident(id), self.0.as_ref(), iter)
    }
}

pub fn render(args: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let mut binary_benchmark = parse2::<BinaryBenchmark>(args)?;
    let item_fn = parse2::<ItemFn>(input)?;
    let cargo_meta = CargoMetadata::try_new();

    binary_benchmark.extract_benches(&item_fn, cargo_meta.as_ref())?;
    if binary_benchmark.benches.is_empty() {
        Ok(binary_benchmark.render_standalone(&item_fn))
    } else {
        Ok(binary_benchmark.render_benches(&item_fn))
    }
}
