use crate::mcp_traits::IdGenerator;
use uuid::Uuid;

/// An [`IdGenerator`] implementation that uses UUID v4 to create unique identifiers.
///
/// This generator produces random UUIDs (version 4), which are highly unlikely
/// to collide and difficult to predict. It is therefore well-suited for
/// generating identifiers such as `SessionId` or other values where uniqueness is important.
pub struct UuidGenerator;

impl<T> IdGenerator<T> for UuidGenerator
where
    T: From<String>,
{
    fn generate(&self) -> T {
        T::from(Uuid::new_v4().to_string())
    }
}
