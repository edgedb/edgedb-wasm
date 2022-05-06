use proc_macro::TokenStream;
use proc_macro_error::emit_error;
use quote::quote;

/// Register web handler
#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn web_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let func_name = &input.sig.ident;
    let hook_name = quote::format_ident!(
        "_edgedb_sdk_init_web_handler_{}", func_name);
    quote! {
        #input

        #[export_name = stringify!(#hook_name)]
        extern fn #hook_name() {
            ::edgedb_sdk::web::register_handler(#func_name);
        }

    }.into()
}

/// Mark function to run at wasm initialization
///
/// In Rust it's uncommon to have init hooks. For most global initialization
/// you can use [`once_cell`]'s or [`lazy_static`]'s.
///
/// There are two cases where init hooks are needed:
///
/// 1. To register handlers. In most cases, more specific registrators should be
///    used though (e.g. [`web_handler`](macro@web_handler)).
/// 2. For smaller latency during request processing (but see below).
///
/// # Influence on Request Latency
///
/// Note: while we will provide request processing latency metric distinct from
/// the initialization time, this may not have the desired effect on users'
/// experience. When request comes in and there is no preinitialized worker,
/// it's likely that user request will need to wait for worker initialization.
/// (We can also employ some techniques to optimize this too).
///
/// [`once_cell`]: https://crates.io/crates/once_cell
/// [`lazy_static`]: https://crates.io/crates/lazy_static
#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn init_hook(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    if !input.sig.generics.params.is_empty() {
        emit_error!(input.sig.generics, "no generics allowed on init hook");
    }
    if !input.sig.inputs.is_empty() {
        emit_error!(input.sig.inputs, "no params allowed on init hook");
    }
    if !matches!(input.sig.output, syn::ReturnType::Default) {
        emit_error!(input.sig.output, "no return value allowed on init hook");
    }
    let func_name = &input.sig.ident;
    let hook_name = quote::format_ident!("_edgedb_sdk_init_{}", func_name);
    quote! {
        #input

        #[export_name = stringify!(#hook_name)]
        extern fn #hook_name() {
            #func_name();
        }

    }.into()
}
