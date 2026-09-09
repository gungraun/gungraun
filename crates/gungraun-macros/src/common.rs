//! spell-checker: ignore punct

use std::fmt::Write;
use std::fs::File as StdFile;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::vec::Vec;

use proc_macro_error3::{abort, emit_error};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, format_ident, quote, quote_spanned};
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{
    Expr, ExprArray, ExprPath, GenericParam, Generics, Ident, LitStr, MetaList, MetaNameValue, Pat,
    Token, parse_quote_spanned, parse2,
};

use crate::CargoMetadata;

#[derive(Debug, Clone)]
pub enum BenchMode {
    Iter(Expr),
    Args(Args),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerfRepetition {
    Dynamic,
    Fixed(Ident),
}

/// This struct reflects the `args` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone)]
pub struct Args(Option<(Span, Vec<Expr>)>);

#[derive(Debug, Clone)]
pub struct Bench {
    pub consts: Option<Consts>,
    pub id: Ident,
    pub mode: BenchMode,
}

/// The `config` parameter of the `#[bench]` or `#[benches]` attribute
#[derive(Debug, Default, Clone)]
pub struct BenchConfig(pub Option<Expr>);

/// This struct stores multiple `Args` as needed by the `#[benches]` attribute
#[derive(Debug, Clone, Default)]
pub struct BenchesArgs {
    pub expected_size: usize,
    pub span: Option<Span>,
    pub values: Option<Vec<Args>>,
}

/// This struct reflects the `args` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone)]
pub struct BenchesConsts {
    pub expected_size: usize,
    pub span: Option<Span>,
    pub values: Option<Vec<Consts>>,
}

/// This struct reflects the `args` parameter of the `#[bench]` attribute
#[derive(Debug, Default, Clone)]
pub struct Consts(Option<(Span, Vec<Expr>)>);

/// The `file` parameter of the `#[benches]` attribute
#[derive(Debug, Default, Clone)]
pub struct File(pub Option<LitStr>);

/// The `iter` parameter of the `#[benches]` attribute
#[derive(Debug, Clone, Default)]
pub struct Iter(pub Option<Expr>);

/// The `setup` parameter
#[derive(Debug, Default, Clone)]
pub struct Setup(pub Option<ExprPath>);

/// The `teardown` parameter
#[derive(Debug, Default, Clone)]
pub struct Teardown(pub Option<ExprPath>);

impl Args {
    pub fn new(span: Span, data: Vec<Expr>) -> Self {
        Self(Some((span, data)))
    }

    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |(_, data)| data.len())
    }

    pub fn span(&self) -> Option<&Span> {
        self.0.as_ref().map(|(span, _)| span)
    }

    pub fn set_span(&mut self, span: Span) {
        if let Some(data) = self.0.as_mut() {
            data.0 = span;
        }
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.0.is_none() {
            let expr = &pair.value;
            let span = expr.span();
            let args = match expr {
                Expr::Array(items) => {
                    let mut args = parse2::<Self>(items.elems.to_token_stream())?;
                    // Set span explicitly (again) to overwrite the wrong span from parse2
                    args.set_span(span);
                    args
                }
                Expr::Tuple(items) => {
                    let mut args = parse2::<Self>(items.elems.to_token_stream())?;
                    // Set span explicitly (again) to overwrite the wrong span from parse2
                    args.set_span(span);
                    args
                }
                Expr::Paren(item) => Self::new(span, vec![(*item.expr).clone()]),
                _ => {
                    #[rustfmt::skip]
                    abort!(
                        expr,
                        "Invalid value for `args`";
                        help = "`args` must be a tuple or array whose elements match the \
                        benchmark function's parameters";
                        note = "#[bench::id(args = (1, 2))]  // tuple
          #[bench::id(args = [1, 2])]  // array"
                    );
                }
            };

            *self = args;
        } else {
            emit_error!(
                pair, "Duplicate parameter: `args`";
                help = "`args` is allowed only once"
            );
        }

        Ok(())
    }

    pub fn parse_meta_list(&mut self, meta: &MetaList) -> syn::Result<()> {
        let mut args = meta.parse_args::<Self>()?;
        args.set_span(meta.tokens.span());

        *self = args;

        Ok(())
    }

    pub fn to_tokens_without_black_box(&self) -> TokenStream {
        if let Some((span, exprs)) = self.0.as_ref() {
            quote_spanned! { *span => #(#exprs),* }
        } else {
            TokenStream::new()
        }
    }

    /// Emit a compiler error if the number of actual and expected arguments do not match
    ///
    /// If there is a setup function present, we do not perform any checks.
    pub fn check_num_arguments(&self, expected: usize, has_setup: bool) {
        let actual = self.len();

        if !has_setup && actual != expected {
            if let Some(span) = self.span() {
                emit_error!(
                    span,
                    "Expected {} arguments but found {}",
                    expected,
                    actual;
                    help = "Number of arguments must match the benchmark function's parameters"
                );
            } else {
                emit_error!(
                    self,
                    "Expected {} arguments but found {}",
                    expected,
                    actual;
                    help = "Number of arguments must match the benchmark function's parameters"
                );
            }
        }
    }
}

impl Parse for Args {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let data = input
            .parse_terminated(Parse::parse, Token![,])?
            .into_iter()
            .collect();

        // We set a default span here although it is most likely wrong. It's strongly advised to set
        // the span with `Args::set_span` to the correct value.
        Ok(Self::new(input.span(), data))
    }
}

impl ToTokens for Args {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some((span, exprs)) = self.0.as_ref() {
            let this_tokens = quote_spanned! { *span => #(std::hint::black_box(#exprs)),* };
            tokens.append_all(this_tokens);
        }
    }
}

impl Bench {
    pub fn new(id: Ident, mode: BenchMode, consts: Option<Consts>) -> Self {
        Self { consts, id, mode }
    }

    /// Return a vector of [`Bench`] parsing the [`File`] or [`BenchesArgs`] if present
    ///
    /// # Aborts
    ///
    /// If there are args in [`BenchesArgs`] and a [`File`] present. We can deal with only one them.
    pub(crate) fn from_benches_attribute(
        fn_span: Span,
        id: &Ident,
        args: BenchesArgs,
        consts: BenchesConsts,
        file: &File,
        iter: &Iter,
        cargo_meta: Option<&CargoMetadata>,
        has_setup: bool,
    ) -> Vec<Self> {
        let expected_num_args = args.expected_size;
        let expected_num_consts = consts.expected_size;

        match (file.is_some(), args.is_some(), iter.is_some()) {
            (true, true, _) | (true, _, true) | (_, true, true) => {
                abort!(
                    id,
                    "Only one parameter of `file`, `args` or `iter` can be present"
                );
            }

            // args is present or none are present in which case we default to args without any
            // elements
            (false, is_args, false) => {
                let args = if is_args {
                    args
                } else {
                    BenchesArgs::default()
                };

                // At least 1 element if neither args nor consts are present but at most number of
                // args or consts depending on which one has more elements.
                let num_elements = (args.len().max(consts.len())).max(1);
                args.iter_infinite()
                    .zip(consts.iter_infinite())
                    .take(num_elements)
                    .enumerate()
                    .map(|(index, (args, consts))| {
                        args.check_num_arguments(expected_num_args, has_setup);
                        if let Some(consts) = consts.as_ref() {
                            consts.check_num_arguments(expected_num_consts);
                        }

                        let id = format_indexed_ident(id, index);
                        Self::new(id, BenchMode::Args(args), consts)
                    })
                    .collect()
            }
            // file is present
            (true, false, false) => {
                let literal = file.literal().expect("file is_some was true");
                if !(expected_num_args == 1 || has_setup) {
                    abort!(
                        literal,
                        "The benchmark function should take exactly one `String` argument if the \
                        file parameter is present";
                        help = "fn benchmark_function(line: String) ..."
                    )
                }

                let strings = file.read(cargo_meta);
                if strings.is_empty() {
                    abort!(literal, "The provided file '{}' was empty", literal.value());
                }

                let mut benches = vec![];
                for (index, (string, consts)) in
                    strings.iter().zip(consts.iter_infinite()).enumerate()
                {
                    let id = format_indexed_ident(id, index);
                    let expr = if string.is_empty() {
                        parse_quote_spanned! { literal.span() => String::new() }
                    } else {
                        parse_quote_spanned! { literal.span() => String::from(#string) }
                    };

                    if let Some(consts) = &consts {
                        consts.check_num_arguments(expected_num_consts);
                    }

                    let args = Args::new(literal.span(), vec![expr]);
                    benches.push(Self::new(id, BenchMode::Args(args), consts));
                }

                benches
            }
            // iter is present
            (false, false, true) => {
                let expr = iter.expr().expect("iter `is_some` should be true");
                if !(expected_num_args == 1 || has_setup) {
                    abort!(
                        fn_span,
                        "The benchmark function can only take exactly one argument if the iter \
                        parameter is present";
                        help = "fn benchmark_function(arg: String) ...";
                        note = "If you need more than one argument you can use a tuple as input and
    destruct it in the function signature. Example:

    #[benches::some_id(iter = vec![(1, 2)])]
    fn benchmark_function((first, second): (u64, u64)) -> usize { ... }"
                    )
                } else if consts.len() > 1 {
                    let span = consts.span.as_ref().unwrap_or(&fn_span);
                    abort!(
                        span,
                        "`iter` requires exactly one `consts` element";
                        help = "`iter` creates multiple benchmarks at runtime, but `consts` \
                        applies const generics at compile time";
                        note = "Use a single consts group: \
                        #[benches::id(iter = ..., consts = [(123, 456)])]"
                    )
                } else {
                    // do nothing
                }

                let first = consts.first(); // There is only one consts or none
                if let Some(first) = &first {
                    first.check_num_arguments(expected_num_consts);
                }

                vec![Self::new(id.clone(), BenchMode::Iter(expr.clone()), first)]
            }
        }
    }
}

impl BenchConfig {
    pub fn ident(id: &Ident) -> Ident {
        format_ident("__get_config", Some(id))
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) {
        if self.0.is_none() {
            self.0 = Some(pair.value.clone());
        } else {
            emit_error!(
                pair, "Duplicate parameter: `config`";
                help = "`config` is allowed only once"
            );
        }
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

impl BenchesArgs {
    pub fn is_some(&self) -> bool {
        self.values.is_some()
    }

    pub fn len(&self) -> usize {
        self.values.as_ref().map_or(0, Vec::len)
    }

    pub fn new(expected_num_args: usize) -> Self {
        Self {
            expected_size: expected_num_args,
            span: None,
            values: None,
        }
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.values.is_none() {
            let values = Self::from_expr(&pair.value)?;
            if values.is_empty() && self.expected_size > 0 {
                #[rustfmt::skip]
                abort!(
                    pair.span(),
                    "Expected the `args` array to contain an element with {} argument(s)",
                    self.expected_size;
                    note = "For example if the benchmark function takes a single parameter:

    /* ... */
    #[benches::id(args = [123])]
    fn benchmark_function(arg: usize) /* ...  */"
                );
            }
            self.span = Some(pair.span());
            self.values = Some(values);
        } else {
            abort!(
                pair, "Duplicate parameter: `args`";
                help = "`args` is allowed only once"
            );
        }

        Ok(())
    }

    pub fn from_expr(expr: &Expr) -> syn::Result<Vec<Args>> {
        let expr_array = parse2::<ExprArray>(expr.to_token_stream())?;
        let mut values: Vec<Args> = vec![];
        for elem in expr_array.elems {
            let span = elem.span();
            let args = match elem {
                Expr::Tuple(items) => {
                    let mut args = parse2::<Args>(items.elems.to_token_stream())?;
                    args.set_span(span);
                    args
                }
                Expr::Paren(item) => Args::new(span, vec![*item.expr]),
                _ => Args::new(span, vec![elem]),
            };

            values.push(args);
        }

        Ok(values)
    }

    pub fn from_meta_list(meta: &MetaList, expected_num_args: usize) -> syn::Result<Self> {
        let list = &meta.tokens;
        let expr = parse2::<Expr>(quote_spanned! { list.span() => [#list] })?;

        let values = Self::from_expr(&expr)?;
        Ok(Self {
            expected_size: expected_num_args,
            span: Some(meta.span()),
            values: Some(values),
        })
    }

    pub fn iter_infinite(self) -> Box<dyn Iterator<Item = Args>> {
        match self.values {
            Some(args) if !args.is_empty() => {
                let last = args
                    .last()
                    .cloned()
                    .expect("There should be a last element");
                Box::new(args.into_iter().chain(std::iter::repeat(last)))
            }
            _ => Box::new(std::iter::repeat(Args::default())),
        }
    }
}

impl BenchesConsts {
    pub(crate) fn parse_pair(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.values.is_none() {
            let values = Self::from_expr(&pair.value)?;
            if values.is_empty() && self.expected_size > 0 {
                #[rustfmt::skip]
                abort!(
                    pair.span(),
                    "Expected the `consts` array to contain an element with {} argument(s)",
                    self.expected_size;
                    note = "For example if the benchmark function takes a single `const` parameter:

    /* ... */
    #[benches::id(consts = [123])]
    fn benchmark_function<const SIZE: usize>() /* ...  */"
                );
            }

            self.span = Some(pair.span());
            self.values = Some(values);
        } else {
            abort!(
                pair, "Duplicate parameter: `consts`";
                help = "`consts` is allowed only once"
            );
        }

        Ok(())
    }

    pub fn from_expr(expr: &Expr) -> syn::Result<Vec<Consts>> {
        let expr_array = parse2::<ExprArray>(expr.to_token_stream())?;
        let mut values: Vec<Consts> = vec![];
        for elem in expr_array.elems {
            let span = elem.span();
            let consts = match elem {
                Expr::Tuple(items) => {
                    let mut consts = parse2::<Consts>(items.elems.to_token_stream())?;
                    consts.set_span(span);
                    consts
                }
                Expr::Paren(item) => Consts::new(span, vec![*item.expr]),
                _ => Consts::new(span, vec![elem]),
            };

            values.push(consts);
        }

        Ok(values)
    }

    pub fn iter_infinite(self) -> Box<dyn Iterator<Item = Option<Consts>>> {
        match self.values {
            Some(consts) if !consts.is_empty() => {
                let last = consts.last().cloned();
                Box::new(consts.into_iter().map(Some).chain(std::iter::repeat(last)))
            }
            _ => Box::new(std::iter::repeat(None)),
        }
    }

    pub fn len(&self) -> usize {
        self.values.as_ref().map_or(0, Vec::len)
    }

    pub fn new(expected_num_consts: usize) -> Self {
        Self {
            span: None,
            expected_size: expected_num_consts,
            values: None,
        }
    }

    pub fn first(self) -> Option<Consts> {
        self.values.into_iter().flatten().nth(0)
    }
}

impl Consts {
    pub fn new(span: Span, data: Vec<Expr>) -> Self {
        Self(Some((span, data)))
    }

    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |(_, data)| data.len())
    }

    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|(_, data)| data.is_empty())
    }

    pub fn exprs(&self) -> Vec<Expr> {
        self.0.as_ref().map_or_else(Vec::new, |(_, v)| v.clone())
    }

    pub fn maybe_string(&self) -> Option<String> {
        self.0.as_ref().and_then(|(_, exprs)| {
            if exprs.is_empty() {
                None
            } else {
                let mut iter = exprs.iter();
                // The unwrap is safe since we just checked that `exprs` is not empty
                let first = iter.next().unwrap().to_token_stream().to_string();
                let string = iter.fold(first, |mut acc, f| {
                    write!(acc, ", {}", f.to_token_stream()).unwrap();
                    acc
                });
                Some(string)
            }
        })
    }

    pub fn span(&self) -> Option<&Span> {
        self.0.as_ref().map(|(span, _)| span)
    }

    pub fn set_span(&mut self, span: Span) {
        if let Some(data) = self.0.as_mut() {
            data.0 = span;
        }
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.0.is_none() {
            let expr = &pair.value;
            let span = expr.span();
            let args = match expr {
                Expr::Array(items) => {
                    let mut args = parse2::<Self>(items.elems.to_token_stream())?;
                    // Set span explicitly (again) to overwrite the wrong span from parse2
                    args.set_span(span);
                    args
                }
                Expr::Tuple(items) => {
                    let mut args = parse2::<Self>(items.elems.to_token_stream())?;
                    // Set span explicitly (again) to overwrite the wrong span from parse2
                    args.set_span(span);
                    args
                }
                Expr::Paren(item) => Self::new(span, vec![(*item.expr).clone()]),
                _ => {
                    #[rustfmt::skip]
                    abort!(
                        expr,
                        "Invalid value for `consts`";
                        help = "`consts` must be a tuple or array whose elements match the \
                        benchmark function's const parameters";
                        note = "#[bench::id(consts = (1, 2))]  // tuple
          #[bench::id(consts = [1, 2])]  // array"
                    );
                }
            };

            *self = args;
        } else {
            emit_error!(
                pair, "Duplicate parameter: `consts`";
                help = "`consts` is allowed only once"
            );
        }

        Ok(())
    }

    /// Emit a compiler error if the number of actual and expected arguments do not match
    ///
    /// If there is a setup function present, we do not perform any checks.
    pub fn check_num_arguments(&self, expected: usize) {
        let actual = self.len();

        if actual != expected {
            if let Some(span) = self.span() {
                emit_error!(
                    span,
                    "Expected {} const arguments but found {}",
                    expected,
                    actual;
                    help = "Number of const expressions must match the benchmark function's \
                    const generic parameters"
                );
            } else {
                emit_error!(
                    self,
                    "Expected {} const arguments but found {}",
                    expected,
                    actual;
                    help = "Number of const expressions must match the benchmark function's \
                    const generic parameters"
                );
            }
        }
    }

    pub fn to_function_calls(
        &self,
        generics: &Generics,
        bench_func_ident: &Ident,
        bench_id: Option<&Ident>,
    ) -> (TokenStream, TokenStream) {
        if self.is_empty() {
            (
                quote! { #bench_func_ident },
                bench_id.map_or_else(TokenStream::new, |ident| quote! { #ident }),
            )
        } else {
            let const_exprs = self.exprs();
            let mut iter_consts = const_exprs.iter();
            let generics_call = generics
                .params
                .iter()
                .filter_map(|g| match g {
                    GenericParam::Lifetime(_) => None,
                    GenericParam::Type(_) => Some(quote! { _ }),
                    GenericParam::Const(c) => {
                        if let Some(next) = iter_consts.next() {
                            Some(next.to_token_stream())
                        } else {
                            abort!(
                                c,
                                "Missing const generic argument";
                                help = "Provide a const expression for each const generic parameter"
                            );
                        }
                    }
                })
                .collect::<Vec<TokenStream>>();
            (
                quote! { #bench_func_ident::<#(#generics_call),*> },
                bench_id.map_or_else(
                    TokenStream::new,
                    |ident| quote! { #ident::<#(#generics_call),*> },
                ),
            )
        }
    }
}

impl Parse for Consts {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let data = input
            .parse_terminated(Parse::parse, Token![,])?
            .into_iter()
            .collect();

        // We set a default span here although it is most likely wrong. It's strongly advised to set
        // the span with `Args::set_span` to the correct value.
        Ok(Self::new(input.span(), data))
    }
}

impl ToTokens for Consts {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some((span, exprs)) = self.0.as_ref() {
            let this_tokens = quote_spanned! { *span => #(#exprs),* };
            tokens.append_all(this_tokens);
        }
    }
}

impl File {
    pub fn literal(&self) -> Option<&LitStr> {
        self.0.as_ref()
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.0.is_none() {
            if let Expr::Lit(literal) = &pair.value {
                self.0 = Some(parse2::<LitStr>(literal.to_token_stream())?);
            } else {
                abort!(
                    pair.value, "Invalid value for `file`";
                    help = "The `file` parameter needs a literal string containing the path to an \
                    existing file at compile time";
                    note = r#"file = "benches/some_fixture""#
                );
            }
        } else {
            emit_error!(
                pair, "Duplicate parameter: `file`";
                help = "`file` is allowed only once"
            );
        }

        Ok(())
    }

    /// Read this [`File`] and return all its lines
    ///
    /// # Panics
    ///
    /// Panics if there is no path present
    pub(crate) fn read(&self, cargo_meta: Option<&CargoMetadata>) -> Vec<String> {
        let expr = self.0.as_ref().expect("A file should be present");
        let string = expr.value();
        let mut path = PathBuf::from(&string);

        if path.is_relative()
            && let Some(cargo_meta) = cargo_meta
        {
            let root = PathBuf::from(&cargo_meta.workspace_root);
            path = root.join(path);
        }

        let file = StdFile::open(&path)
            .unwrap_or_else(|error| abort!(expr, "Error opening '{}': {}", path.display(), error));

        let mut lines = vec![];
        for (index, line) in BufReader::new(file).lines().enumerate() {
            match line {
                Ok(line) => {
                    lines.push(line);
                }
                Err(error) => {
                    abort!(
                        self.0,
                        "Error reading line {} in file '{}': {}",
                        index + 1,
                        path.display(),
                        error
                    );
                }
            }
        }
        lines
    }
}

impl Iter {
    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub fn expr(&self) -> Option<&Expr> {
        self.0.as_ref()
    }

    pub fn parse_pair(&mut self, pair: &MetaNameValue) {
        if self.0.is_none() {
            self.0 = Some(pair.value.clone());
        } else {
            emit_error!(
                pair, "Duplicate parameter: `iter`";
                help = "`iter` is allowed only once"
            );
        }
    }
}

impl Setup {
    pub fn parse_pair(&mut self, pair: &MetaNameValue) {
        if self.0.is_none() {
            let expr = &pair.value;
            if let Expr::Path(path) = expr {
                self.0 = Some(path.clone());
            } else {
                abort!(
                    expr, "Invalid value for `setup`";
                    help = "The `setup` parameter needs a path to an existing function \
                    in a reachable scope";
                    note = "`setup = my_setup` or `setup = my::setup::function`"
                );
            }
        } else {
            abort!(
                pair, "Duplicate parameter: `setup`";
                help = "`setup` is allowed only once"
            );
        }
    }

    pub fn to_string_with_args(&self, args: &Args) -> String {
        let tokens = args.to_tokens_without_black_box();
        if let Some(setup) = self.0.as_ref() {
            format!("{}({tokens})", setup.to_token_stream())
        } else {
            tokens.to_string()
        }
    }

    pub fn to_string_with_iter(&self, iter: &Expr) -> String {
        let tokens = iter.to_token_stream();
        if let Some(setup) = self.0.as_ref() {
            format!("{}(nth of {tokens})", setup.to_token_stream())
        } else {
            format!("nth of {tokens}")
        }
    }

    /// If this Setup is none and the other setup has a value update this `Setup` with that value
    pub fn update(&mut self, other: &Self) {
        if let (None, Some(other)) = (&self.0, &other.0) {
            self.0 = Some(other.clone());
        }
    }
}

impl Teardown {
    pub fn parse_pair(&mut self, pair: &MetaNameValue) {
        if self.0.is_none() {
            let expr = &pair.value;
            if let Expr::Path(path) = expr {
                self.0 = Some(path.clone());
            } else {
                abort!(
                    expr, "Invalid value for `teardown`";
                    help = "The `teardown` parameter needs a path to an existing function \
                    in a reachable scope";
                    note = "`teardown = my_teardown` or `teardown = my::teardown::function`"
                );
            }
        } else {
            abort!(
                pair, "Duplicate parameter: `teardown`";
                help = "`teardown` is allowed only once"
            );
        }
    }

    /// If this Teardown is none and the other Teardown has a value update this Teardown with that
    /// value
    pub fn update(&mut self, other: &Self) {
        if let (None, Some(other)) = (&self.0, &other.0) {
            self.0 = Some(other.clone());
        }
    }
}

pub fn format_ident(prefix: &str, ident: Option<&Ident>) -> Ident {
    if let Some(ident) = ident {
        format_ident!("{prefix}_{ident}")
    } else {
        format_ident!("{prefix}")
    }
}

pub fn format_indexed_ident(ident: &Ident, index: usize) -> Ident {
    format_ident!("{ident}_{index}")
}

pub fn pattern_to_single_function_ident(
    pat: &Pat,
    elem_ident: &Ident,
    index: usize,
) -> Option<Pat> {
    match pat {
        Pat::Ident(pat_ident) => Some(Pat::Ident(syn::PatIdent {
            attrs: pat_ident.attrs.clone(),
            by_ref: None,
            mutability: None,
            ident: pat_ident.ident.clone(),
            subpat: None,
        })),
        Pat::Paren(pat_paren) => {
            pattern_to_single_function_ident(&pat_paren.pat, elem_ident, index)
        }
        Pat::Reference(pat_reference) => Some(Pat::Reference(syn::PatReference {
            pat: Box::new(pattern_to_single_function_ident(
                &pat_reference.pat,
                elem_ident,
                index,
            )?),
            ..pat_reference.clone()
        })),
        Pat::Slice(pat_slice) => Some(Pat::Ident(syn::PatIdent {
            attrs: pat_slice.attrs.clone(),
            by_ref: None,
            mutability: None,
            ident: format_ident!("{elem_ident}_{index}"),
            subpat: None,
        })),
        Pat::Struct(pat_struct) => Some(Pat::Ident(syn::PatIdent {
            attrs: pat_struct.attrs.clone(),
            by_ref: None,
            mutability: None,
            ident: format_ident!("{elem_ident}_{index}"),
            subpat: None,
        })),
        Pat::Tuple(pat_tuple) => Some(Pat::Ident(syn::PatIdent {
            attrs: pat_tuple.attrs.clone(),
            by_ref: None,
            mutability: None,
            ident: format_ident!("{elem_ident}_{index}"),
            subpat: None,
        })),
        Pat::TupleStruct(pat_tuple_struct) => Some(Pat::Ident(syn::PatIdent {
            attrs: pat_tuple_struct.attrs.clone(),
            by_ref: None,
            mutability: None,
            ident: format_ident!("{elem_ident}_{index}"),
            subpat: None,
        })),
        Pat::Wild(pat_wild) => Some(Pat::Ident(syn::PatIdent {
            attrs: pat_wild.attrs.clone(),
            by_ref: None,
            mutability: None,
            ident: format_ident!("{elem_ident}_{index}"),
            subpat: None,
        })),
        Pat::Path(_) => Some(pat.clone()),
        _ => None,
    }
}

/// Truncate a utf-8 [`std::str`] to a given `len`
pub fn truncate_str_utf8(string: &str, len: usize) -> &str {
    if let Some((pos, c)) = string
        .char_indices()
        .take_while(|(i, c)| i + c.len_utf8() <= len)
        .last()
    {
        &string[..pos + c.len_utf8()]
    } else {
        &string[..0]
    }
}
