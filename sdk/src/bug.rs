//! Experimental way to report program/SDK bugs
//!
//! This is mostly used instead of panics where appropriate (for example in
//! assertions), as panics are extremely costly in WebAssembly (i.e. we restart
//! the VM after panic occurs)

/// This errors means bug in SDK or environment running WebAssembly
#[derive(thiserror::Error, Debug)]
#[error("bug detected")]
pub struct Bug {
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

/// Extension trait for the [`wrap_bug`](BugContext::wrap_bug) method.
pub trait BugContext<T, E>  {
    fn wrap_bug(self) -> Result<T, Bug>;
}

impl<T, E> BugContext<T, E> for Result<T, E>
    where E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>
{
    fn wrap_bug(self) -> Result<T, Bug> {
        self.map_err(|e| {
            Bug {
                source: Some(e.into()),
            }
        })
    }
}
