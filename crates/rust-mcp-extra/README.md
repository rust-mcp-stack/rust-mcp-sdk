# rust-mcp-extra

**A companion crate to [`rust-mcp-sdk`](https://github.com/rust-mcp-stack/rust-mcp-sdk) providing additional implementations for core traits like `IdGenerator`, `SessionStore` and `EventStore`.**

-----
## 🔢 ID Generators
Various implementations of the IdGenerator<T> trait (from [rust-mcp-sdk]) for generating unique identifiers.

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
