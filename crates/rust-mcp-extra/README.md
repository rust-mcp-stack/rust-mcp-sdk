# rust-mcp-extra

A companion crate to [`rust-mcp-sdk`](https://github.com/rust-mcp-stack/rust-mcp-sdk) providing additional implementations for core traits like `AuthProvider`, `IdGenerator`, `SessionStore` and `EventStore`.

## 📖 Table of Contents
- **[Authentication Providers](#-authentication-providers)**
  - [Keycloak](#keycloak)
  - [WorkOs Authkit](#workos-authkit)
  - [Scalekit](#scalekit)
- **[ID Generators](#-id-generators)**
  - [NanoIdGenerator](#nanoidgenerator)
  - [TimeBase64Generator](#timebase64generator)
  - [RandomBase62Generator](#randombase62generator)
  - [SnowflakeIdGenerator](#snowflakeidgenerator)
- **[Session Stores](#-session-stores)**
  - 🔜 Coming Soon
- **[Event Stores](#-event-stores)**
  - 🔜 Coming Soon


-----
## 🔐 Authentication Providers
A collection of authentication providers that integrate seamlessly with the [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk).
These providers offer a ready-to-use integration with common identity systems, so developers don’t have to build an AuthProvider implementation for each provider themselves.


### **Keycloak**
A full OAuth2/OpenID Connect provider integration for [Keycloak](https://www.keycloak.org) backed identity systems.
Useful for enterprise environments or self-hosted identity setups.

- Example usage:

```rs
let auth_provider = KeycloakAuthProvider::new(KeycloakAuthOptions {
    keycloak_base_url: "http://localhost:8080/realms/master".to_string(),
    mcp_server_url: "http://localhost:3000".to_string(),
    resource_name: Some("Keycloak Oauth Test MCP Server".to_string()),
    required_scopes: None,
    client_id: "keycloak-client-id".to_string(),
    client_secret: "keycloak-client-secret".to_string(),
    token_verifier: None,
    resource_documentation: None,
})?;
```

Before running the [example](../../examples/keycloak-auth.rs), ensure you have a Keycloak instance properly configured.  
Follow the official MCP authorization tutorial for Keycloak setup:[Keycloak Setup Guide](https://modelcontextprotocol.io/docs/tutorials/security/authorization#keycloak-setup)

By default, the example assumes Keycloak is running at `http://localhost:8080`.

configure a confidential client in Keycloak and provide credentials as environment variables:

```sh
export AUTH_SERVER=http://localhost:8080/realms/master
export CLIENT_ID=your-confidential-client-id
export CLIENT_SECRET=your-client-secret
cargo run -p rust-mcp-extra --example keycloak-auth
```


### **WorkOS AuthKit**
An OAuth provider implementation for [WorkOS Authkit](https://workos.com).

- Example usage:

```rs
let auth_provider = WorkOsAuthProvider::new(WorkOSAuthOptions {
        authkit_domain: "https://stalwart-opera-85-staging.authkit.app".to_string(),
        mcp_server_url: "http://127.0.0.1:3000/mcp".to_string(),
        required_scopes: Some(vec!["openid", "profile"]),
        resource_name: Some("Workos Oauth Test MCP Server".to_string()),
        resource_documentation: None,
        token_verifier: None,
    })?;
```

Before running the [example](../../examples/workos-auth.rs), make sure you enabled DCR (Dynamic Client Regsitration) in your WorkOS Authkit dashboard.

Set the `AUTH_SERVER` environment variable and start the example:

```
export AUTH_SERVER=https://stalwart-opera-85-staging.authkit.app
cargo run -p rust-mcp-extra --example workos-auth
```



### **Scalekit**
An OAuth provider implementation for [Scalekit](https://www.scalekit.com).

- Example usage:

```rs
let auth_provider = ScalekitAuthProvider::new(ScalekitAuthOptions {
    mcp_server_url: "http://127.0.0.1:3000/mcp".to_string(),
    required_scopes: Some(vec!["profile"]),
    token_verifier: None,
    resource_name: Some("Scalekit Oauth Test MCP Server".to_string()),
    resource_documentation: None,
    environment_url: "yourapp.scalekit.dev".to_string(),
    resource_id: "res_your-resource_id".to_string(),
})
.await?;
```

Set the `ENVIRONMENT_URL` and `RESOURCE_ID` environment variable and start the example:

```
export ENVIRONMENT_URL=yourapp.scalekit.dev
export RESOURCE_ID=res_your-resource_id
cargo run -p rust-mcp-extra --example scalekit-auth
```



## 🔢 ID Generators
Various implementations of the IdGenerator trait (from [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk)) for generating unique identifiers.

| **🧩 All ID generators in this crate can be used as `SessionId` generators in [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk)).**


| Generator |  Description|
| -------------- |  ----- |
| **NanoIdGenerator**                | Generates short, URL-safe, random string IDs using the [`nanoid`](https://crates.io/crates/nanoid) crate. Ideal for user-friendly, compact identifiers.                       |
| **TimeBase64Generator**     | Encodes the current timestamp (in milliseconds) into a URL-safe Base64 string. Useful when IDs should be time-sortable and compact.                                           |
| **RandomBase62Generator**  | Generates alphanumeric [A–Z, a–z, 0–9] strings using random bytes. A simple, reliable option for random unique IDs.                                                           |
| **SnowflakeIdGenerator**    | Inspired by Twitter’s Snowflake algorithm. Generates 64-bit time-ordered IDs containing timestamp, machine ID, and sequence. Best for distributed or high-throughput systems. |


### How to use
Provide an instance of your chosen ID generator in the **HyperServerOptions** when initializing the server.

For example to use **SnowflakeIdGenerator** :

```rs
use rust_mcp_extra::id_generator::SnowflakeIdGenerator;


let server = hyper_server::create_server(
    server_details,
    handler,
    HyperServerOptions {
        host: "127.0.0.1".to_string(),
        session_id_generator: Some(Arc::new(SnowflakeIdGenerator::new(1015))), // use SnowflakeIdGenerator
        ..Default::default()
    },
);

```

-----

## 💾 Session Stores

`SessionStore` implementations are available for managing MCP sessions effectively.

🔜 Coming Soon

-----

## 💽 Event Stores
`EventStore` implementations to enable resumability on MCP servers by reliably storing and replaying event histories.

🔜 Coming Soon
