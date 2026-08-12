//! Procedural macros for json-steroids
//!
//! Generates efficient serializers and deserializers for data structures.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, Type};

/// Get the crate path - resolves whether we are inside
/// the `json_steroids` crate itself or an external consumer.
fn crate_path() -> TokenStream2 {
    match crate_name("json-steroids") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident }
        }
        // Fallback: we are inside the crate being compiled (unit tests, benchmarks)
        Err(_) => quote! { crate },
    }
}

/// Enum for `#[json(default=..)]` field attribute
#[derive(Clone)]
enum FieldDefault {
    None,              // no default value
    Default,           // `#[json(default)]` - uses Default::default()
    Custom(syn::Path), // `#[json(default=custom_function)]`
}

/// Container-level attributes
#[derive(Clone)]
struct ContainerAttrs {
    rename_all: Option<String>,
}

/// Field-level attributes
#[derive(Clone)]
struct FieldAttrs {
    rename: Option<String>,
    default: FieldDefault,
    serialize_with: Option<syn::Path>,
    deserialize_with: Option<syn::Path>,
    skip_serializing: bool,
    skip_deserializing: bool,
    aliases: Vec<String>,
}

fn validate_case(case: &str) -> bool {
    matches!(
        case,
        "lowercase"
            | "UPPERCASE"
            | "PascalCase"
            | "camelCase"
            | "snake_case"
            | "SCREAMING_SNAKE_CASE"
            | "kebab-case"
            | "SCREAMING-KEBAB-CASE"
    )
}

/// Split `identifier` by words. Used by [`apply_case`] function.
fn split_by_words(identifier: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = identifier.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '_' {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
        } else if c.is_uppercase() {
            if !current.is_empty() {
                let prev_is_lower = current.chars().last().is_some_and(|ch| ch.is_lowercase());
                let next_is_lower = chars.peek().is_some_and(|ch| ch.is_lowercase());

                if prev_is_lower || next_is_lower {
                    words.push(current.clone());
                    current.clear();
                }
            }
            current.push(c.to_lowercase().next().unwrap());
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Case converter for `#[json(rename_all = ...)]` attribute
fn apply_case(identifier: &str, case: &str) -> String {
    let words = split_by_words(identifier);
    match case {
        "lowercase" => words.join("").to_lowercase(),
        "UPPERCASE" => words.join("").to_uppercase(),
        "PascalCase" => words
            .iter()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect(),
        "camelCase" => {
            let mut res = String::new();
            for (i, w) in words.iter().enumerate() {
                if i == 0 {
                    res.push_str(&w.to_lowercase());
                } else {
                    let mut c = w.chars();
                    if let Some(f) = c.next() {
                        res.push_str(&f.to_uppercase().collect::<String>());
                        res.push_str(c.as_str());
                    }
                }
            }
            res
        }
        "snake_case" => words.join("_").to_lowercase(),
        "SCREAMING_SNAKE_CASE" => words.join("_").to_uppercase(),
        "kebab-case" => words.join("-").to_lowercase(),
        "SCREAMING-KEBAB-CASE" => words.join("-").to_uppercase(),
        _ => words.join(""),
    }
}

/// Extracts a `syn::Path` from either a string literal or a path expression.
/// This allows attribute values with and without double quotes
/// (i.e. `deserialize_with = "my_module::func"` and `deserialize_with = my_module::func`)
fn parse_path_from_expr(
    expr: &syn::Expr,
    attr_name: &str,
    field_name: &str,
) -> syn::Result<syn::Path> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) => lit_str.parse::<syn::Path>().map_err(|_| {
            syn::Error::new_spanned(
                lit_str,
                format!(
                    "Field {field_name} has an invalid value {} for a `json` attribute {attr_name}",
                    lit_str.value(),
                ),
            )
        }),
        syn::Expr::Path(syn::ExprPath { path, .. }) => Ok(path.clone()),
        _ => Err(syn::Error::new_spanned(
            expr,
            format!("field `{field_name}` has an invalid value for a `json` attribute flag `{attr_name}`: expected string literal or path expression"),
        )),
    }
}

/// Helper to extract the attribute name from a path.
fn get_attr_name(path: &syn::Path) -> String {
    path.get_ident()
        .map(|id| id.to_string())
        .unwrap_or_else(|| quote!(#path).to_string().replace(' ', ""))
}

/// Parses field level `#[json(...)]` helper attributes.
fn parse_field_attrs(attrs: &[syn::Attribute], field_name: &str) -> syn::Result<FieldAttrs> {
    let mut field_attrs = FieldAttrs {
        rename: None,
        default: FieldDefault::None,
        serialize_with: None,
        deserialize_with: None,
        skip_serializing: false,
        skip_deserializing: false,
        aliases: Vec::new(),
    };

    for attr in attrs {
        if attr.path().is_ident("json") {
            let nested = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )?;
            for meta in nested {
                match meta {
                    syn::Meta::Path(path) => {
                        let attr_ident = path.get_ident().map(|i| i.to_string());
                        match attr_ident.as_deref() {
                            Some("default") => {
                                if !matches!(field_attrs.default, FieldDefault::None) {
                                    return Err(syn::Error::new_spanned(
                                    &path,
                                    format!("field `{field_name}` has a duplicate `json` attribute flag: `default`")
                                ));
                                }
                                field_attrs.default = FieldDefault::Default;
                            }
                            Some("skip") => {
                                if field_attrs.skip_serializing || field_attrs.skip_deserializing {
                                    return Err(syn::Error::new_spanned(
                                    &path,
                                    format!("field `{field_name}` has a duplicate or conflicting `json` attribute flag: `skip`")
                                ));
                                }
                                field_attrs.skip_serializing = true;
                                field_attrs.skip_deserializing = true;
                            }
                            Some("skip_serializing") => {
                                if field_attrs.skip_serializing {
                                    return Err(syn::Error::new_spanned(
                                    &path,
                                    format!("field `{field_name}` has a duplicate `json` attribute flag: `skip_serializing`"),
                                ));
                                }
                                field_attrs.skip_serializing = true;
                            }
                            Some("skip_deserializing") => {
                                if field_attrs.skip_deserializing {
                                    return Err(syn::Error::new_spanned(
                                    &path,
                                    format!( "field `{field_name}` has a duplicate `json` attribute flag: `skip_deserializing`"),
                                ));
                                }
                                field_attrs.skip_deserializing = true;
                            }
                            _ => {
                                let attr_name = get_attr_name(&path);
                                return Err(syn::Error::new_spanned(
                                &path,
                                format!("field `{field_name}` has an unsupported `json` attribute: {attr_name}"),
                            ));
                            }
                        }
                    }
                    syn::Meta::NameValue(nv) => {
                        let attr_ident = nv.path.get_ident().map(|i| i.to_string());
                        match attr_ident.as_deref() {
                            Some("rename") => {
                                if field_attrs.rename.is_some() {
                                    return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("field `{field_name}` has a duplicate `json` attribute flag: `rename`"),
                                ));
                                }
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(lit_str),
                                    ..
                                }) = &nv.value
                                {
                                    field_attrs.rename = Some(lit_str.value());
                                } else {
                                    return Err(syn::Error::new_spanned(
                                    &nv.value,
                                    format!("field `{field_name}` has an invalid value for the `json` attribute flag `rename`: expected string literal"),
                                ));
                                }
                            }
                            Some("alias") => {
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(lit_str),
                                    ..
                                }) = &nv.value
                                {
                                    field_attrs.aliases.push(lit_str.value());
                                } else {
                                    return Err(syn::Error::new_spanned(
                                    &nv.value,
                                    format!("field `{field_name}` has an invalid value for the `json` attribute flag `alias`: expected string literal"),
                                ));
                                }
                            }
                            Some("default") => {
                                if !matches!(field_attrs.default, FieldDefault::None) {
                                    return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("field `{field_name}` has a duplicate `json` attribute flag: `default`"),
                                ));
                                }
                                let path = parse_path_from_expr(&nv.value, "default", field_name)?;
                                field_attrs.default = FieldDefault::Custom(path);
                            }
                            Some("serialize_with") => {
                                if field_attrs.serialize_with.is_some() {
                                    return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("field `{field_name}` has a duplicate `json` attribute flag: `serialize_with`"),
                                ));
                                }
                                let path =
                                    parse_path_from_expr(&nv.value, "serialize_with", field_name)?;
                                field_attrs.serialize_with = Some(path);
                            }
                            Some("deserialize_with") => {
                                if field_attrs.deserialize_with.is_some() {
                                    return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("field `{field_name}` has a duplicate `json` attribute flag: `deserialize_with`"),
                                ));
                                }
                                let path = parse_path_from_expr(
                                    &nv.value,
                                    "deserialize_with",
                                    field_name,
                                )?;
                                field_attrs.deserialize_with = Some(path);
                            }
                            Some("with") => {
                                if field_attrs.serialize_with.is_some()
                                    || field_attrs.deserialize_with.is_some()
                                {
                                    return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("field `{field_name}` has a duplicate or conflicting `json` attribute flag: `with`")
                                ));
                                }
                                let path = parse_path_from_expr(&nv.value, "with", field_name)?;
                                let mut ser_path = path.clone();
                                ser_path.segments.push(
                                    syn::Ident::new("serialize", proc_macro2::Span::call_site())
                                        .into(),
                                );
                                let mut de_path = path;
                                de_path.segments.push(
                                    syn::Ident::new("deserialize", proc_macro2::Span::call_site())
                                        .into(),
                                );
                                field_attrs.serialize_with = Some(ser_path);
                                field_attrs.deserialize_with = Some(de_path);
                            }
                            _ => {
                                let attr_name = get_attr_name(&nv.path);
                                return Err(syn::Error::new_spanned(
                                &nv.path,
                                format!("field `{field_name}` has an unsupported `json` attribute: {attr_name}"),
                            ));
                            }
                        }
                    }
                    syn::Meta::List(list) => {
                        // we don't support attrs in the form of #[json(attr(...))]
                        let attr_name = get_attr_name(&list.path);
                        return Err(syn::Error::new_spanned(
                            &list,
                            format!("field `{field_name}` has an unsupported `json` attribute: {attr_name}"),
                        ));
                    }
                }
            }
        }
    }

    Ok(field_attrs)
}

fn parse_container_attrs(attrs: &[syn::Attribute]) -> syn::Result<ContainerAttrs> {
    let mut container_attrs = ContainerAttrs { rename_all: None };

    for attr in attrs {
        if attr.path().is_ident("json") {
            let nested = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )?;
            for meta in nested {
                match meta {
                    syn::Meta::NameValue(nv) => {
                        let attr_ident = nv.path.get_ident().map(|i| i.to_string());
                        match attr_ident.as_deref() {
                            Some("rename_all") => {
                                if container_attrs.rename_all.is_some() {
                                    return Err(syn::Error::new_spanned(
                                        &nv.path,
                                        "duplicate `json` attribute flag: `rename_all`",
                                    ));
                                }
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(lit_str),
                                    ..
                                }) = &nv.value
                                {
                                    let val = lit_str.value();
                                    if !validate_case(&val) {
                                        return Err(syn::Error::new_spanned(
                                            &nv.value,
                                            format!("invalid value for `rename_all`: `{}`. Expected one of: lowercase, UPPERCASE, PascalCase, camelCase, snake_case, SCREAMING_SNAKE_CASE, kebab-case, SCREAMING-KEBAB-CASE", val),
                                        ));
                                    }
                                    container_attrs.rename_all = Some(val);
                                } else {
                                    return Err(syn::Error::new_spanned(
                                        &nv.value,
                                        "invalid value for the `json` attribute flag `rename_all`: expected string literal",
                                    ));
                                }
                            }
                            _ => {
                                let attr_name = get_attr_name(&nv.path);
                                return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("unsupported `json` container attribute: {attr_name}"),
                                ));
                            }
                        }
                    }
                    syn::Meta::Path(path) => {
                        let attr_name = get_attr_name(&path);
                        return Err(syn::Error::new_spanned(
                            &path,
                            format!("unsupported `json` container attribute: {attr_name}"),
                        ));
                    }
                    syn::Meta::List(list) => {
                        let attr_name = get_attr_name(&list.path);
                        return Err(syn::Error::new_spanned(
                            &list,
                            format!("unsupported `json` container attribute: {attr_name}"),
                        ));
                    }
                }
            }
        }
    }
    Ok(container_attrs)
}

/// Derive macro for JSON serialization
///
/// # Example
/// ```ignore
/// #[derive(JsonSerialize)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
/// ```
#[proc_macro_derive(JsonSerialize, attributes(json))]
pub fn derive_json_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_derive_json_serialize(input)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}

fn expand_derive_json_serialize(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let krate = crate_path();

    let container_attrs = parse_container_attrs(&input.attrs)?;
    let rename_all = container_attrs.rename_all.as_deref();

    let serialize_body = generate_serialize_body(&input.data, name, &krate, rename_all)?;

    Ok(quote! {
        impl #impl_generics #krate::JsonSerialize for #name #ty_generics #where_clause {
            fn json_serialize<W: #krate::writer::Writer>(&self, writer: &mut #krate::JsonWriter<W>) {
                #serialize_body
            }
        }
    })
}

/// Derive macro for JSON deserialization
///
/// # Example
/// ```ignore
/// #[derive(JsonDeserialize)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
/// ```
#[proc_macro_derive(JsonDeserialize, attributes(json))]
pub fn derive_json_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_derive_json_deserialize(input)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}

fn expand_derive_json_deserialize(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = &input.generics;
    let krate = crate_path();

    let container_attrs = parse_container_attrs(&input.attrs)?;
    let rename_all = container_attrs.rename_all.as_deref();

    let deserialize_body = generate_deserialize_body(&input.data, name, &krate, rename_all)?;

    // Add 'de lifetime to generics only if it doesn't already exist
    let mut generics_with_de = generics.clone();
    let has_de_lifetime = generics.lifetimes().any(|lt| lt.lifetime.ident == "de");
    if !has_de_lifetime {
        generics_with_de.params.insert(0, syn::parse_quote!('de));
    }
    let (impl_generics, _, _) = generics_with_de.split_for_impl();
    let (_, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #krate::JsonDeserialize<'de> for #name #ty_generics #where_clause {
            fn json_deserialize(parser: &mut #krate::JsonParser<'de>) -> #krate::Result<Self> {
                #deserialize_body
            }
        }
    })
}

/// Combined derive for both serialization and deserialization
#[proc_macro_derive(Json, attributes(json))]
pub fn derive_json(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_derive_json(input)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}

fn expand_derive_json(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = &input.generics;
    let krate = crate_path();

    let container_attrs = parse_container_attrs(&input.attrs)?;
    let rename_all = container_attrs.rename_all.as_deref();

    let serialize_body = generate_serialize_body(&input.data, name, &krate, rename_all)?;
    let deserialize_body = generate_deserialize_body(&input.data, name, &krate, rename_all)?;

    // For serialize: use normal generics
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // For deserialize: add 'de lifetime only if it doesn't already exist
    let mut generics_with_de = generics.clone();
    let has_de_lifetime = generics.lifetimes().any(|lt| lt.lifetime.ident == "de");
    if !has_de_lifetime {
        generics_with_de.params.insert(0, syn::parse_quote!('de));
    }
    let (impl_generics_de, _, _) = generics_with_de.split_for_impl();

    Ok(quote! {
        impl #impl_generics #krate::JsonSerialize for #name #ty_generics #where_clause {
            fn json_serialize<W: #krate::writer::Writer>(&self, writer: &mut #krate::JsonWriter<W>) {
                #serialize_body
            }
        }

        impl #impl_generics_de #krate::JsonDeserialize<'de> for #name #ty_generics #where_clause {
            fn json_deserialize(parser: &mut #krate::JsonParser<'de>) -> #krate::Result<Self> {
                #deserialize_body
            }
        }
    })
}

fn generate_serialize_fields_named(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    get_accessor: impl Fn(&Ident) -> TokenStream2,
    krate: &TokenStream2,
    rename_all: Option<&str>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut field_serializations = Vec::new();
    let mut first = true;
    for f in fields {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let attrs = parse_field_attrs(&f.attrs, &field_name_str)?;
        if attrs.skip_serializing {
            continue;
        }

        let json_key = if let Some(rename) = attrs.rename {
            rename
        } else if let Some(case) = rename_all {
            apply_case(&field_name_str, case)
        } else {
            field_name_str.clone()
        };

        let field_accessor = get_accessor(field_name);

        let write_value = if let Some(ser_fn) = attrs.serialize_with {
            quote! { #ser_fn(#field_accessor, writer); }
        } else {
            quote! { #krate::JsonSerialize::json_serialize(#field_accessor, writer); }
        };

        let comma = if first {
            first = false;
            quote! {}
        } else {
            quote! { writer.write_comma(); }
        };

        field_serializations.push(quote! {
            #comma
            writer.write_unescape_key(#json_key);
            #write_value
        });
    }
    Ok(field_serializations)
}

fn generate_serialize_fields_unnamed(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    get_accessor: impl Fn(usize) -> TokenStream2,
    krate: &TokenStream2,
    _rename_all: Option<&str>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut field_serializations = Vec::new();
    let mut first = true;
    for (idx, f) in fields.iter().enumerate() {
        let field_name_str = idx.to_string();
        let attrs = parse_field_attrs(&f.attrs, &field_name_str)?;
        if attrs.skip_serializing {
            continue;
        }

        let field_accessor = get_accessor(idx);

        let write_value = if let Some(ser_fn) = attrs.serialize_with {
            quote! { #ser_fn(#field_accessor, writer); }
        } else {
            quote! { #krate::JsonSerialize::json_serialize(#field_accessor, writer); }
        };

        let comma = if first {
            first = false;
            quote! {}
        } else {
            quote! { writer.write_comma(); }
        };

        field_serializations.push(quote! {
            #comma
            #write_value
        });
    }
    Ok(field_serializations)
}

fn generate_serialize_body(
    data: &Data,
    _name: &Ident,
    krate: &TokenStream2,
    rename_all: Option<&str>,
) -> syn::Result<TokenStream2> {
    match data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => {
                let field_serializations = generate_serialize_fields_named(
                    &fields.named,
                    |name| quote! { &self.#name },
                    krate,
                    rename_all,
                )?;

                Ok(quote! {
                    writer.begin_object();
                    #(#field_serializations)*
                    writer.end_object();
                })
            }

            Fields::Unnamed(fields) => {
                let field_serializations = generate_serialize_fields_unnamed(
                    &fields.unnamed,
                    |idx| {
                        let idx = syn::Index::from(idx);
                        quote! { &self.#idx }
                    },
                    krate,
                    rename_all,
                )?;

                Ok(quote! {
                    writer.begin_array();
                    #(#field_serializations)*
                    writer.end_array();
                })
            }

            Fields::Unit => Ok(quote! {
                writer.write_null();
            }),
        },

        Data::Enum(data_enum) => {
            let mut variants = Vec::with_capacity(data_enum.variants.len());

            for variant in &data_enum.variants {
                let variant_name = &variant.ident;
                let variant_name_str = variant_name.to_string();
                let variant_attrs = parse_field_attrs(&variant.attrs, &variant_name_str)?;

                if variant_attrs.skip_serializing {
                    continue;
                }

                let variant_json_key = if let Some(rename) = variant_attrs.rename {
                    rename
                } else if let Some(case) = rename_all {
                    apply_case(&variant_name_str, case)
                } else {
                    variant_name_str.clone()
                };

                match &variant.fields {
                    Fields::Unit => {
                        variants.push(quote! {
                            Self::#variant_name => {
                                writer.write_string(#variant_json_key);
                            }
                        });
                    }

                    Fields::Unnamed(fields) => {
                        let field_names: Vec<Ident> = (0..fields.unnamed.len())
                            .map(|i| format_ident!("f{}", i))
                            .collect();

                        let mut pattern_fields = Vec::new();

                        for (idx, f) in fields.unnamed.iter().enumerate() {
                            let field_name_str = idx.to_string();
                            let attrs = parse_field_attrs(&f.attrs, &field_name_str)?;

                            if attrs.skip_serializing {
                                pattern_fields.push(quote! { _ });
                            } else {
                                let name = &field_names[idx];
                                pattern_fields.push(quote! { #name });
                            }
                        }

                        let field_serializations = generate_serialize_fields_unnamed(
                            &fields.unnamed,
                            |idx| {
                                let name = &field_names[idx];
                                quote! { #name }
                            },
                            krate,
                            rename_all,
                        )?;

                        variants.push(quote! {
                            Self::#variant_name(#(#pattern_fields),*) => {
                                writer.begin_object();
                                writer.write_unescape_key(#variant_json_key);
                                writer.begin_array();
                                #(#field_serializations)*
                                writer.end_array();
                                writer.end_object();
                            }
                        });
                    }

                    Fields::Named(fields) => {
                        let mut used_field_names = Vec::new();

                        for f in &fields.named {
                            let field_name = f.ident.as_ref().unwrap();
                            let field_name_str = field_name.to_string();
                            let attrs = parse_field_attrs(&f.attrs, &field_name_str)?;

                            if !attrs.skip_serializing {
                                used_field_names.push(field_name);
                            }
                        }

                        let field_serializations = generate_serialize_fields_named(
                            &fields.named,
                            |name| quote! { #name },
                            krate,
                            rename_all,
                        )?;

                        let pattern = if used_field_names.len() == fields.named.len() {
                            quote! {
                                Self::#variant_name { #(#used_field_names),* }
                            }
                        } else if used_field_names.is_empty() {
                            quote! {
                                Self::#variant_name { .. }
                            }
                        } else {
                            quote! {
                                Self::#variant_name { #(#used_field_names),*, .. }
                            }
                        };

                        variants.push(quote! {
                            #pattern => {
                                writer.begin_object();
                                writer.write_unescape_key(#variant_json_key);
                                writer.begin_object();
                                #(#field_serializations)*
                                writer.end_object();
                                writer.end_object();
                            }
                        });
                    }
                }
            }

            Ok(quote! {
                match self {
                    #(#variants)*
                }
            })
        }

        Data::Union(data_union) => Err(syn::Error::new_spanned(
            data_union.union_token,
            "Unions are not supported by json-steroids",
        )),
    }
}

struct NamedFieldDe {
    declaration: TokenStream2,
    match_arm: Option<TokenStream2>,
    unwrap: TokenStream2,
}

fn generate_named_field_de(
    f: &syn::Field,
    krate: &TokenStream2,
    rename_all: Option<&str>,
) -> syn::Result<NamedFieldDe> {
    let field_name = f.ident.as_ref().unwrap();
    let field_name_str = field_name.to_string();
    let attrs = parse_field_attrs(&f.attrs, &field_name_str)?;
    let is_option = is_option_type(&f.ty);

    if attrs.skip_deserializing {
        let unwrap = match &attrs.default {
            FieldDefault::Default => quote! {
                #field_name: ::core::default::Default::default()
            },

            FieldDefault::Custom(path) => quote! {
                #field_name: #path()
            },

            FieldDefault::None => {
                if is_option {
                    quote! {
                        #field_name: None
                    }
                } else {
                    quote! {
                        #field_name: ::core::default::Default::default()
                    }
                }
            }
        };

        return Ok(NamedFieldDe {
            declaration: quote! {},
            match_arm: None,
            unwrap,
        });
    }

    let declaration = quote! { let mut #field_name = None; };

    let json_key = if let Some(rename) = attrs.rename {
        rename
    } else if let Some(case) = rename_all {
        apply_case(&field_name_str, case)
    } else {
        field_name_str.clone()
    };

    let read_value = if let Some(de_fn) = &attrs.deserialize_with {
        quote! { #de_fn(parser)? }
    } else {
        quote! { #krate::JsonDeserialize::json_deserialize(parser)? }
    };

    let mut match_arms = vec![quote! {
        #json_key => {
            #field_name = Some(#read_value);
        }
    }];

    for alias in &attrs.aliases {
        match_arms.push(quote! {
            #alias => {
                #field_name = Some(#read_value);
            }
        });
    }

    let match_arm = Some(quote! {
        #(#match_arms)*
    });

    let unwrap = match &attrs.default {
        FieldDefault::Default => quote! {
            #field_name: #field_name.unwrap_or_default()
        },

        FieldDefault::Custom(path) => quote! {
            #field_name: #field_name.unwrap_or_else(|| #path())
        },

        FieldDefault::None => {
            if is_option {
                quote! {
                    #field_name: #field_name.unwrap_or(None)
                }
            } else {
                quote! {
                    #field_name: #field_name.ok_or_else(|| {
                        #krate::JsonError::MissingField(#json_key.to_string())
                    })?
                }
            }
        }
    };

    Ok(NamedFieldDe {
        declaration,
        match_arm,
        unwrap,
    })
}

fn generate_unnamed_field_de(
    f: &syn::Field,
    var_name: &Ident,
    idx: usize,
    first: &mut bool,
    krate: &TokenStream2,
) -> syn::Result<TokenStream2> {
    let field_name_str = idx.to_string();
    let attrs = parse_field_attrs(&f.attrs, &field_name_str)?;

    if attrs.skip_deserializing {
        let default_expr = match &attrs.default {
            FieldDefault::Default => quote! { ::core::default::Default::default() },
            FieldDefault::Custom(path) => quote! { #path() },
            FieldDefault::None => quote! { ::core::default::Default::default() },
        };
        return Ok(quote! { let #var_name = #default_expr; });
    }

    let read_value = if let Some(de_fn) = attrs.deserialize_with {
        quote! { #de_fn(parser)? }
    } else {
        quote! { #krate::JsonDeserialize::json_deserialize(parser)? }
    };

    let comma = if *first {
        *first = false;
        quote! {}
    } else {
        quote! { parser.expect_comma()?; }
    };

    Ok(quote! {
        #comma
        let #var_name = #read_value;
    })
}

fn generate_deserialize_body(
    data: &Data,
    name: &Ident,
    krate: &TokenStream2,
    rename_all: Option<&str>,
) -> syn::Result<TokenStream2> {
    match data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => {
                let mut field_declarations = Vec::with_capacity(fields.named.len());
                let mut field_matches = Vec::new();
                let mut field_unwraps = Vec::with_capacity(fields.named.len());

                for f in &fields.named {
                    let de = generate_named_field_de(f, krate, rename_all)?;
                    field_declarations.push(de.declaration);
                    if let Some(m) = de.match_arm {
                        field_matches.push(m);
                    }
                    field_unwraps.push(de.unwrap);
                }

                Ok(quote! {
                    parser.expect_object_start()?;
                    #(#field_declarations)*

                    while let Some(key) = parser.next_object_key()? {
                        match key.as_ref() {
                            #(#field_matches)*
                            _ => { parser.skip_value()?; }
                        }
                    }

                    parser.expect_object_end()?;

                    Ok(#name {
                        #(#field_unwraps),*
                    })
                })
            }
            Fields::Unnamed(fields) => {
                let field_vars: Vec<Ident> = (0..fields.unnamed.len())
                    .map(|i| format_ident!("f{}", i))
                    .collect();

                let mut field_deserializations = Vec::new();
                let mut first = true;
                for (i, f) in fields.unnamed.iter().enumerate() {
                    let var_name = &field_vars[i];
                    let stmt = generate_unnamed_field_de(f, var_name, i, &mut first, krate)?;
                    field_deserializations.push(stmt);
                }

                Ok(quote! {
                    parser.expect_array_start()?;
                    #(#field_deserializations)*
                    parser.expect_array_end()?;
                    Ok(#name(#(#field_vars),*))
                })
            }
            Fields::Unit => Ok(quote! {
                parser.expect_null()?;
                Ok(#name)
            }),
        },
        Data::Enum(data_enum) => {
            let mut unit_string_matches = Vec::new();
            let mut object_variant_matches = Vec::new();

            for variant in &data_enum.variants {
                let variant_name = &variant.ident;
                let variant_name_str = variant_name.to_string();
                let variant_attrs = parse_field_attrs(&variant.attrs, &variant_name_str)?;

                if variant_attrs.skip_deserializing {
                    continue;
                }

                let variant_json_key = if let Some(rename) = variant_attrs.rename {
                    rename
                } else if let Some(case) = rename_all {
                    apply_case(&variant_name_str, case)
                } else {
                    variant_name_str.clone()
                };

                match &variant.fields {
                    Fields::Unit => {
                        unit_string_matches.push(quote! {
                            #variant_json_key => Ok(#name::#variant_name),
                        });
                        object_variant_matches.push(quote! {
                            #variant_json_key => {
                                parser.expect_null()?;
                                parser.expect_object_end()?;
                                Ok(#name::#variant_name)
                            },
                        });
                    }
                    Fields::Unnamed(fields) => {
                        let field_vars: Vec<Ident> = (0..fields.unnamed.len())
                            .map(|i| format_ident!("f{}", i))
                            .collect();

                        let mut field_deserializations = Vec::new();
                        let mut first = true;
                        for (i, f) in fields.unnamed.iter().enumerate() {
                            let var_name = &field_vars[i];
                            let stmt =
                                generate_unnamed_field_de(f, var_name, i, &mut first, krate)?;
                            field_deserializations.push(stmt);
                        }

                        object_variant_matches.push(quote! {
                            #variant_json_key => {
                                parser.expect_array_start()?;
                                #(#field_deserializations)*
                                parser.expect_array_end()?;
                                parser.expect_object_end()?;
                                Ok(#name::#variant_name(#(#field_vars),*))
                            },
                        });
                    }
                    Fields::Named(fields) => {
                        let mut field_declarations = Vec::with_capacity(fields.named.len());
                        let mut field_matches = Vec::new();
                        let mut field_unwraps = Vec::with_capacity(fields.named.len());

                        for f in &fields.named {
                            let de = generate_named_field_de(f, krate, rename_all)?;
                            field_declarations.push(de.declaration);
                            if let Some(m) = de.match_arm {
                                field_matches.push(m);
                            }
                            field_unwraps.push(de.unwrap);
                        }

                        object_variant_matches.push(quote! {
                            #variant_json_key => {
                                parser.expect_object_start()?;
                                #(#field_declarations)*

                                while let Some(key) = parser.next_object_key()? {
                                    match key.as_ref() {
                                        #(#field_matches)*
                                        _ => { parser.skip_value()?; }
                                    }
                                }

                                parser.expect_object_end()?;
                                parser.expect_object_end()?;

                                Ok(#name::#variant_name {
                                    #(#field_unwraps),*
                                })
                            },
                        });
                    }
                }
            }

            Ok(quote! {
                if parser.peek_is_string()? {
                    let variant_str = parser.parse_string()?;
                    match variant_str.as_ref() {
                        #(#unit_string_matches)*
                        _ => Err(#krate::JsonError::UnknownVariant(variant_str.to_string()))
                    }
                } else {
                    parser.expect_object_start()?;
                    let key = parser.next_object_key()?.ok_or(#krate::JsonError::UnexpectedEnd)?;
                    match key.as_ref() {
                        #(#object_variant_matches)*
                        _ => Err(#krate::JsonError::UnknownVariant(key.to_string()))
                    }
                }
            })
        }
        Data::Union(data_union) => Err(syn::Error::new_spanned(
            data_union.union_token,
            "Unions are not supported by json-steroids",
        )),
    }
}

/// Checks if a type represents `Option<T>`
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}
