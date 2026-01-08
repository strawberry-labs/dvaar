//! Local web inspector for debugging HTTP requests through the tunnel

pub mod client;
mod html;
pub mod port;
mod server;
mod store;

pub use client::InspectorClient;
pub use port::{find_inspector_port, InspectorMode};
pub use server::start_server;
pub use store::{CapturedRequest, RegisteredTunnel, RequestStore, TunnelStatus};
