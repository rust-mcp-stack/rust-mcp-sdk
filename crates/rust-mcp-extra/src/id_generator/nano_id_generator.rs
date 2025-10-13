//! Short (Smaller than UUID), URL-safe, Customizable alphabet, Cryptographically secure

use nanoid::nanoid;
use rust_mcp_sdk::id_generator::IdGenerator;

/// A NanoID-based ID generator that produces short, URL-safe, unique strings.
///
/// This generator is well-suited for cases where:
/// - You want compact, human-friendly IDs
/// - UUIDs are too long or verbose
/// - You don't need time-based or ordered IDs
///
/// Internally uses the `nanoid` crate to generate secure, random IDs.
///
/// # Example
/// ```
/// use rust_mcp_extra::{id_generator::NanoIdGenerator,IdGenerator};
///
/// let generator = NanoIdGenerator::new(10);
/// let id: String = generator.generate();
/// println!("Generated ID: {}", id);
/// assert_eq!(id.len(), 10);
/// ```
pub struct NanoIdGenerator {
    size: usize, // number of characters in the ID
}

impl NanoIdGenerator {
    /// Creates a new Nano ID generator.
    ///
    /// # Arguments
    /// * `size` - Length of the generated ID (default: 21 if unsure)
    pub fn new(size: usize) -> Self {
        Self { size }
    }
}

impl<T> IdGenerator<T> for NanoIdGenerator
where
    T: From<String>,
{
    fn generate(&self) -> T {
        let size = self.size;
        let id = nanoid!(size);
        T::from(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_correct_length_id() {
        let generator = NanoIdGenerator::new(12);
        let id: String = generator.generate();
        assert_eq!(id.len(), 12);
    }

    #[test]
    fn generates_unique_ids() {
        let generator = NanoIdGenerator::new(8);
        let mut seen = std::collections::HashSet::new();

        for _ in 0..1000 {
            let id: String = generator.generate();
            assert!(seen.insert(id.clone()), "Duplicate ID: {}", id);
        }
    }
}
