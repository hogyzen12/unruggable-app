// Titan Exchange integration module
// Provides WebSocket-based swap quote streaming using MessagePack protocol

pub mod types;
pub mod codec;
pub mod client;
pub mod transaction_builder;

#[cfg(test)]
pub mod test;

pub use client::TitanClient;
pub use types::*;
pub use transaction_builder::build_transaction_from_route;