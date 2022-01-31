use proc_macro::TokenStream;
use proc_macro_error::emit_error;
use quote::quote;

#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn web_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
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
