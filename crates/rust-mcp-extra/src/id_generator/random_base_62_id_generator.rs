//! Short, URL-safe, No collisions if length is sufficient
//! Needs collision handling if critical

use rand::Rng;
use rand_distr::Alphanumeric;
use rust_mcp_sdk::id_generator::IdGenerator;

/// A random Base62 ID generator.
///
/// Generates short, random alphanumeric strings composed of [A-Z, a-z, 0-9].
/// Useful when you want compact, URL-safe random IDs without needing
/// time-based ordering.
///
/// # Example
/// ```
/// use rust_mcp_extra::{id_generator::RandomBase62Generator,IdGenerator};
///
/// let generator = RandomBase62Generator::new(12);
/// let id: String = generator.generate();
/// println!("Generated Base62 ID: {}", id);
/// ```
pub struct RandomBase62Generator {
    size: usize,
}

impl RandomBase62Generator {
    /// Creates a new random Base62 ID generator.
    ///
    /// # Arguments
    /// * `size` - Length of the generated ID.
    pub fn new(size: usize) -> Self {
        Self { size }
    }
}

impl<T> IdGenerator<T> for RandomBase62Generator
where
    T: From<String>,
{
    /// Generates a new random Base62 ID string.
    ///
    /// The ID consists of randomly selected alphanumeric characters (A-Z, a-z, 0-9).
    fn generate(&self) -> T {
        let id: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(self.size)
            .map(char::from)
            .collect();

        T::from(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_non_empty_id() {
        let generator = RandomBase62Generator::new(16);
        let id: String = generator.generate();
        assert_eq!(id.len(), 16);
        assert!(!id.is_empty());
    }

    #[test]
    fn generates_unique_ids() {
        let generator = RandomBase62Generator::new(8);
        let mut seen = std::collections::HashSet::new();

        for _ in 0..1000 {
            let id: String = generator.generate();
            assert!(seen.insert(id), "Duplicate ID generated");
        }
    }

    #[test]
    fn only_alphanumeric_characters() {
        let generator = RandomBase62Generator::new(50);
        let id: String = generator.generate();

        assert!(
            id.chars().all(|c| c.is_ascii_alphanumeric()),
            "ID contains non-alphanumeric chars"
        );
    }
}
