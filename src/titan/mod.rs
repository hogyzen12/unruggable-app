// Titan Exchange integration module
// Provides WebSocket-based swap quote streaming using MessagePack protocol

pub mod client;
pub mod codec;
pub mod transaction_builder;
pub mod types;

#[cfg(test)]
pub mod test;

pub use client::TitanClient;
pub use transaction_builder::build_transaction_from_route;
pub use types::*;
