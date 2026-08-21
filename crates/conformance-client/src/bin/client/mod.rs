//! Implementation modules for the `conformance-client` binary.
//!
//! - [`handler`]: implements `ClientHandler` (elicitation, sampling) with
//!   "echo"-style default responses suitable for the conformance suite.
//! - [`transport`]: builds the streamable-HTTP transport and starts a
//!   client runtime.
//! - [`scenarios`]: one module per non-auth scenario (`initialize`,
//!   `tools_call`, `elicitation`, `sse_retry`).
//! - [`auth`]: orchestrates the full OAuth flow used by every `auth/*`
//!   scenario, with sub-modules for each phase (discovery, token
//!   acquisition, scope step-up).

pub mod auth;
pub mod handler;
pub mod scenarios;
pub mod transport;
