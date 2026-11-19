//! Shared building blocks for the LAN Wake Manager controller, relay and agent.
pub mod client;
pub mod cmd;
pub mod diag;
pub mod discovery;
pub mod jobs;
pub mod logging;
pub mod models;
pub mod net;
pub mod paths;
pub mod power;
pub mod probe;
pub mod ratelimit;
pub mod schedule;
pub mod server;
pub mod service;
pub mod sys;
pub mod tls;
pub mod util;
pub mod wol;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const API_VERSION: u32 = 1;
