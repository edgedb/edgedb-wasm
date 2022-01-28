use proc_macro::TokenStream;

#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn web_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    let stream = proc_macro2::TokenStream::from(item);
    // TODO(tailhook) register this item in global registry using init_hook
    stream.into()
}

#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn init_hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    let stream = proc_macro2::TokenStream::from(item);
    // TODO(tailhook) make a special export named _edgedb_sdk_init_xxx for this
    stream.into()
}
