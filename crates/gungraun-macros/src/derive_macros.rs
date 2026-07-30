use proc_macro_error3::abort;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse2;

pub fn render_into_inner(input_stream: TokenStream) -> syn::Result<TokenStream> {
    let input: syn::DeriveInput = parse2(input_stream)?;
    let src = &input.ident;
    let inner = if let syn::Data::Struct(item) = &input.data {
        if let syn::Fields::Unnamed(fields) = &item.fields {
            let unnamed: Vec<syn::Field> = fields
                .unnamed
                .pairs()
                .map(|pair| pair.into_value().clone())
                .collect();
            match unnamed.as_slice() {
                [] => abort!(input, "Tuple struct must have exactly one field";
                    help = "Add a field to the tuple struct"
                ),
                [field] => field.clone(),
                _ => abort!(input, "Tuple struct must have exactly one field";
                    help = "Use a tuple struct with a single field: `struct Name(Field);`"
                ),
            }
        } else {
            abort!(input, "Only tuple structs with unnamed fields are allowed";
                help = "Use a tuple struct: `struct Name(Field);`"
            )
        }
    } else {
        abort!(input, "Only structs are allowed";
            help = "Use a struct definition instead of an enum, union, or other item"
        );
    };

    let expanded = quote! {
        impl From<#src> for #inner {
            fn from(value: #src) -> Self {
                value.0
            }
        }

        impl From<&#src> for #inner {
            fn from(value: &#src) -> Self {
                value.0.clone()
            }
        }

        impl From<&mut #src> for #inner {
            fn from(value: &mut #src) -> Self {
                value.0.clone()
            }
        }
    };

    Ok(expanded)
}
