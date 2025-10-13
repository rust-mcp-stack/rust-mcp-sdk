# rust-mcp-extra

**A companion crate to [`rust-mcp-sdk`](https://github.com/rust-mcp-stack/rust-mcp-sdk) providing additional implementations for core traits like `IdGenerator`, `SessionStore` and `EventStore`.**


## 🔢 ID Generators
This crate provides several implementations of the IdGenerator<T> trait (from [rust-mcp-sdk]) for generating unique identifiers.

| **🧩 All ID generators in this crate can be used as `SessionId` generators in [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk)).**


| Generator|  Description|
| ----- |  ----- |
| **NanoIdGenerator**                | Generates short, URL-safe, random string IDs using the [`nanoid`](https://crates.io/crates/nanoid) crate. Ideal for user-friendly, compact identifiers.                       |
| **TimeBase64Generator**     | Encodes the current timestamp (in milliseconds) into a URL-safe Base64 string. Useful when IDs should be time-sortable and compact.                                           |
| **RandomBase62Generator**  | Generates alphanumeric [A–Z, a–z, 0–9] strings using random bytes. A simple, reliable option for random unique IDs.                                                           |
| **SnowflakeIdGenerator**    | Inspired by Twitter’s Snowflake algorithm. Generates 64-bit time-ordered IDs containing timestamp, machine ID, and sequence. Best for distributed or high-throughput systems. |
