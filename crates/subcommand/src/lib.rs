use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::IntoIter, Data, DeriveInput, Fields, Ident,
    Path, PathSegment, Type, TypePath,
};

/// Derives a `dispatch(self) -> anyhow::Result<()>` async method on a
/// clap `Subcommand` enum.
///
/// Convention: each variant must be a single-field tuple whose type is
/// `some::module::SubArgs`.  The last path segment (`SubArgs`) is stripped
/// and `::run` is appended, so `Stats(stats::SubArgs)` dispatches to
/// `stats::run(args).await`.
///
/// Nested subcommands work automatically: the inner `SubArgs` just needs
/// its own `Command` field that also `#[derive(Dispatch)]`.
#[proc_macro_derive(Dispatch)]
pub fn derive_dispatch(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let enum_name: &Ident = &input.ident;

    let Data::Enum(data_enum) = &input.data else {
        return syn::Error::new_spanned(
            &input.ident,
            "Dispatch only works on enums",
        )
        .to_compile_error()
        .into();
    };

    let arms: Vec<TokenStream2> = data_enum
        .variants
        .iter()
        .map(|variant| {
            let variant_name: &Ident = &variant.ident;

            // Each variant must be a single unnamed field: Foo(some::path::SubArgs)
            let Fields::Unnamed(fields) = &variant.fields else {
                return syn::Error::new_spanned(
                    variant_name,
                    "Dispatch requires single-field tuple variants, e.g. Stats(stats::SubArgs)",
                )
                .to_compile_error();
            };

            if fields.unnamed.len() != 1 {
                return syn::Error::new_spanned(
                    variant_name,
                    "Dispatch requires exactly one field per variant",
                )
                .to_compile_error();
            }

            let field_ty: &Type = &fields.unnamed[0].ty;
            let Type::Path(TypePath { path, .. }) = field_ty else {
                return syn::Error::new_spanned(
                    field_ty,
                    "Dispatch requires a plain path type (e.g. stats::SubArgs)",
                )
                .to_compile_error();
            };

            let module_path: Path = module_from_type_path(path);

            quote! {
                #enum_name::#variant_name(args) => #module_path::run(args).await,
            }
        })
        .collect();

    quote! {
        impl #enum_name {
            pub async fn dispatch(self) -> ::anyhow::Result<()> {
                match self {
                    #(#arms)*
                }
            }
        }
    }
    .into()
}

/// Strips the last path segment from `some::module::SubArgs`
/// and returns `some::module` as a `Path`.
fn module_from_type_path(path: &Path) -> Path {
    let mut module: Path = path.clone();
    module.segments.pop(); // remove SubArgs

    // pop() leaves a trailing `::` punctuation — remove it cleanly
    if let Some(last) = module.segments.last_mut() {
        // segments are Pair::Punctuated or Pair::End; force to End
        let _ = last; // already owned after pop/re-push; syn handles it
    }

    // Rebuild without trailing punct by collecting into a new path
    let segs: IntoIter<PathSegment> = module.segments.into_iter();
    syn::parse_quote! { #(#segs)::* }
}
