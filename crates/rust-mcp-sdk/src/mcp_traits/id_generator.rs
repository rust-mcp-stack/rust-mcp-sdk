/// Trait for generating unique identifiers.
///
/// This trait is generic over the target ID type, allowing it to be used for
/// generating different kinds of identifiers such as `SessionId` or
/// transport-scoped `StreamId`.
///
pub trait IdGenerator<T>: Send + Sync
where
    T: From<String>,
{
    fn generate(&self) -> T;
}
