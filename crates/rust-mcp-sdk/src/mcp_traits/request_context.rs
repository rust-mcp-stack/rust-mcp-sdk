use rust_mcp_schema::{
    schema_utils::RpcErrorCodes, ClientCapabilities, Implementation, JsonObject, LoggingLevel,
    ProgressToken, ProtocolVersion, RequestMetaObject, RpcError,
};

/// A client capability that a server handler may require.
///
/// Before dispatching a request, the runtime checks the client's
/// declared capabilities against the handler's requirements.  If a
/// required capability is missing the request is rejected with
/// [`crate::schema::MISSING_REQUIRED_CLIENT_CAPABILITY`] (-32021).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequiredClientCapability {
    /// The client must support sampling (`sampling` in `ClientCapabilities`).
    Sampling,
    /// The client must support elicitation (`elicitation`).
    Elicitation,
    /// The client must support roots (`roots`).
    Roots,
    /// The client must declare support for a specific extension.
    Extension(&'static str),
}

impl RequiredClientCapability {
    /// Human-readable name shown in error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sampling => "sampling",
            Self::Elicitation => "elicitation",
            Self::Roots => "roots",
            Self::Extension(key) => key,
        }
    }

    /// Check whether the given client capabilities satisfy this requirement.
    pub fn is_satisfied_by(&self, caps: &ClientCapabilities) -> bool {
        match self {
            Self::Sampling => caps.sampling.is_some(),
            Self::Elicitation => caps.elicitation.is_some(),
            Self::Roots => caps.roots.is_some(),
            Self::Extension(key) => caps
                .extensions
                .as_ref()
                .is_some_and(|ext| ext.contains_key(*key)),
        }
    }

    /// The `ClientCapabilities`-shaped JSON fragment naming this capability,
    /// used in `MissingRequiredClientCapabilityError.data.requiredCapabilities`
    /// (the schema defines it as an object of capability objects, e.g.
    /// `{ "sampling": {} }`, not an array of names).
    fn as_capability_json(&self) -> (&'static str, serde_json::Value) {
        match self {
            Self::Extension(_) => (self.as_str(), serde_json::json!({})),
            _ => (self.as_str(), serde_json::json!({})),
        }
    }
}

/// Per-request context extracted from `RequestMetaObject` and carried to every handler.
///
/// The 2026-07-28 protocol has no `initialize` handshake: protocol version, client identity,
/// capabilities and optional progress/log-level hints are declared in `_meta` on every request.
/// Servers MUST NOT infer these values from prior requests.
pub struct RequestContext {
    /// The negotiated protocol version for this request.
    pub protocol_version: ProtocolVersion,
    /// The client's capabilities (required per spec).
    pub client_capabilities: ClientCapabilities,
    /// The client's self-reported identity (SHOULD per spec).
    pub client_info: Option<Implementation>,
    /// Opaque token for progress notifications, if the caller requests them.
    pub progress_token: Option<ProgressToken>,
    /// Desired log level for this request (SEP-2577 deprecated; may be absent or ignored).
    #[allow(deprecated)]
    pub log_level: Option<LoggingLevel>,
}

impl RequestContext {
    /// Builds a validated `RequestContext` from the wire `RequestMetaObject`.
    ///
    /// Returns `UnsupportedProtocolVersionError` (-32022) when the protocol
    /// version is unknown to this implementation or is a known version the
    /// server has chosen not to support (see
    /// `crate::utils::supported_protocol_versions`). The error data carries
    /// the `supported` versions and echoes the `requested` version.
    pub fn from_request_meta(meta: &RequestMetaObject) -> Result<Self, RpcError> {
        let unsupported = || {
            RpcError::new(
                RpcErrorCodes::UNSUPPORTED_PROTOCOL_VERSION,
                format!("Unsupported protocol version '{}'", meta.protocol_version),
                Some(serde_json::json!({
                    "supported": crate::utils::supported_protocol_versions(),
                    "requested": meta.protocol_version,
                })),
            )
        };

        let protocol_version =
            ProtocolVersion::try_from(meta.protocol_version.as_str()).map_err(|_| unsupported())?;

        if !crate::utils::supported_protocol_versions().contains(&meta.protocol_version) {
            return Err(unsupported());
        }

        Ok(Self {
            protocol_version,
            client_capabilities: meta.client_capabilities.clone(),
            client_info: meta.client_info.clone(),
            progress_token: meta.progress_token.clone(),
            #[allow(deprecated)]
            log_level: meta.log_level.clone(),
        })
    }

    /// Enforce that the client declared every required capability.
    ///
    /// Returns `Ok(())` when all requirements are met, otherwise an error
    /// with code [`crate::schema::MISSING_REQUIRED_CLIENT_CAPABILITY`] (-32021) whose
    /// `data.requiredCapabilities` is a `ClientCapabilities`-shaped object
    /// keyed by each missing capability (e.g. `{ "sampling": {} }`).
    pub fn ensure_capabilities(
        &self,
        method: &str,
        required: &[RequiredClientCapability],
    ) -> Result<(), RpcError> {
        let missing: Vec<_> = required
            .iter()
            .filter(|c| !c.is_satisfied_by(&self.client_capabilities))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }

        let missing_names: Vec<_> = missing.iter().map(|c| c.as_str()).collect();
        let required_capabilities: serde_json::Map<String, serde_json::Value> = missing
            .iter()
            .map(|c| {
                let (key, value) = c.as_capability_json();
                (key.to_string(), value)
            })
            .collect();

        Err(RpcError::new(
            RpcErrorCodes::MISSING_REQUIRED_CLIENT_CAPABILITY,
            format!(
                "Client must declare {missing_names:?} capability/capabilities to call '{method}'",
            ),
            Some(serde_json::json!({
                "requiredCapabilities": required_capabilities,
            })),
        ))
    }

    /// Returns the client's declared extensions, if any.
    pub fn client_extensions(&self) -> Option<&std::collections::BTreeMap<String, JsonObject>> {
        self.client_capabilities.extensions.as_ref()
    }

    /// A minimal context for custom/non-standard requests that carry no
    /// `RequestMetaObject`. Uses the current compiled protocol version
    /// and empty capabilities.
    pub fn empty() -> Self {
        Self {
            protocol_version: ProtocolVersion::latest(),
            client_capabilities: ClientCapabilities::default(),
            client_info: None,
            progress_token: None,
            #[allow(deprecated)]
            log_level: None,
        }
    }
}
