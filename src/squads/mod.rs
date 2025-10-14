// src/squads/mod.rs
//! Squads v4 multisig integration
//! 
//! This module provides integration with Squads v4 multisig protocol,
//! following the existing patterns in the unruggable app for transactions
//! and signing operations.

pub mod client;
pub mod types;

pub use client::SquadsClient;
pub use types::*;