//! Local web inspector for debugging HTTP requests through the tunnel

mod html;
mod server;
mod store;

pub use server::start_server;
pub use store::{CapturedRequest, RequestStore};
