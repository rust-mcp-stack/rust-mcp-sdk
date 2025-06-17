#[cfg(feature = "2025_03_26")]
pub use rust_mcp_schema::*;

#[cfg(all(feature = "2024_11_05", not(any(feature = "2025_03_26"))))]
pub use rust_mcp_schema::mcp_2024_11_05::*;
