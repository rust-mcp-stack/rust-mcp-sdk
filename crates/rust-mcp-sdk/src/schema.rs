#[cfg(feature = "2025_06_18")]
pub use rust_mcp_schema::*;

#[cfg(not(feature = "2025_06_18"))]
pub use rust_mcp_schema::{ParseProtocolVersionError, ProtocolVersion};

#[cfg(all(
    feature = "2025_03_26",
    not(any(feature = "2024_11_05", feature = "2025_06_18"))
))]
pub use rust_mcp_schema::mcp_2025_03_26::*;

#[cfg(all(
    feature = "2024_11_05",
    not(any(feature = "2025_03_26", feature = "2025_06_18"))
))]
pub use rust_mcp_schema::mcp_2024_11_05::*;
