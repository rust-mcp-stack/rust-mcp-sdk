// Copyright (c) 2025 mcp-rust-stack
// Licensed under the MIT License. See LICENSE file for details.
// Modifications to this file must be documented with a description of the changes made.
#[cfg(feature = "sse")]
mod client_sse;
pub mod error;
mod mcp_stream;
mod message_dispatcher;
mod schema;
#[cfg(feature = "sse")]
mod sse;
mod stdio;
mod transport;
mod utils;

#[cfg(feature = "sse")]
pub use client_sse::*;
pub use message_dispatcher::*;
#[cfg(feature = "sse")]
pub use sse::*;
pub use stdio::*;
pub use transport::*;

// Type alias for session identifier, represented as a String
pub type SessionId = String;
