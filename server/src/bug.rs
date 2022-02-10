use std::fmt;

pub struct Bug {
    description: String,
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

pub trait Context<T, E>  {
    fn wrap_bug(self, description: impl ToString) -> Result<T, Bug>;
}

impl<T, E> Context<T, E> for Result<T, E>
    where E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>
{
    fn wrap_bug(self, description: impl ToString) -> Result<T, Bug> {
        self.map_err(|e| {
            Bug {
                description: description.to_string(),
                source: Some(e.into()),
            }
        })
    }
}

impl<T> Context<T, std::convert::Infallible> for Option<T>
{
    fn wrap_bug(self, description: impl ToString) -> Result<T, Bug> {
        self.ok_or_else(|| {
            Bug {
                description: description.to_string(),
                source: None,
            }
        })
    }
}

impl fmt::Display for Bug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(src) = &self.source {
            write!(f, "internal error: {}: {}", self.description, src)
        } else {
            write!(f, "internal error: {}", self.description)
        }
    }
}
