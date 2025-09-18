// Copyright (c) 2025 mcp-rust-stack
// Licensed under the MIT License. See LICENSE file for details.
// Modifications to this file must be documented with a description of the changes made.

#[cfg(feature = "sse")]
mod client_sse;
#[cfg(feature = "streamable-http")]
mod client_streamable_http;
mod constants;
pub mod error;
pub mod event_store;
mod mcp_stream;
mod message_dispatcher;
mod schema;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
mod sse;
#[cfg(feature = "stdio")]
mod stdio;
mod transport;
mod utils;

#[cfg(feature = "sse")]
pub use client_sse::*;
#[cfg(feature = "streamable-http")]
pub use client_streamable_http::*;
pub use constants::*;
pub use message_dispatcher::*;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub use sse::*;
#[cfg(feature = "stdio")]
pub use stdio::*;
pub use transport::*;

// Type alias for session identifier, represented as a String
pub type SessionId = String;
// Type alias for stream identifier (that will be used at the transport scope), represented as a String
pub type StreamId = String;
// Type alias for event (MCP message) identifier, represented as a String
pub type EventId = String;
