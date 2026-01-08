//! Dvaar Common - Shared protocol library for the tunneling service
//!
//! This crate contains the protocol definitions and serialization helpers
//! used by both the server and CLI.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Protocol errors
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Failed to serialize message: {0}")]
    Serialize(#[from] rmp_serde::encode::Error),

    #[error("Failed to deserialize message: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),

    #[error("Invalid message format")]
    InvalidFormat,
}

/// Control packet - the main message type for tunnel communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPacket {
    /// Sent by client immediately after WebSocket connection is established
    Init(ClientHello),

    /// Server response to Init
    InitAck(ServerHello),

    /// HTTP request from server to client (client should forward to local upstream)
    /// Body is streamed separately via Data packets, followed by End
    HttpRequest(HttpRequestPacket),

    /// HTTP response from client to server
    /// Body is streamed separately via Data packets, followed by End
    HttpResponse(HttpResponsePacket),

    /// Raw data chunk (bidirectional) - used for streaming request/response bodies
    Data {
        stream_id: String,
        data: Vec<u8>,
    },

    /// End of stream signal - marks completion of request/response body
    End {
        stream_id: String,
    },

    /// WebSocket frame passthrough (for HMR, real-time features)
    WebSocketFrame {
        stream_id: String,
        data: Vec<u8>,
        is_binary: bool,
    },

    /// WebSocket connection closed
    WebSocketClose {
        stream_id: String,
        code: Option<u16>,
        reason: Option<String>,
    },

    /// Stream error - signals an error on a specific stream
    StreamError {
        stream_id: String,
        error: String,
    },

    /// Keepalive ping
    Ping,

    /// Keepalive pong
    Pong,
}

/// Initial handshake from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    /// Authentication token
    pub token: String,

    /// Optional requested subdomain (if None, server assigns random)
    pub requested_subdomain: Option<String>,

    /// Type of tunnel
    pub tunnel_type: TunnelType,

    /// Client version for compatibility checking
    pub client_version: String,
}

/// Server response to client handshake
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    /// The assigned domain (e.g., "cool-app.dvaar.app")
    pub assigned_domain: String,

    /// Error message if authentication or subdomain request failed
    pub error: Option<String>,

    /// Server version
    pub server_version: String,
}

/// Type of tunnel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelType {
    /// HTTP/HTTPS tunnel
    Http,

    /// Raw TCP tunnel (optional feature)
    Tcp,
}

impl TunnelType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            TunnelType::Http => "HTTP",
            TunnelType::Tcp => "TCP",
        }
    }
}

/// HTTP request packet sent from server to client
/// Note: Body is streamed separately via Data packets followed by End packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestPacket {
    /// Unique identifier for this request/response pair
    pub stream_id: String,

    /// HTTP method (GET, POST, etc.)
    pub method: String,

    /// Request path including query string
    pub uri: String,

    /// HTTP headers as key-value pairs
    pub headers: Vec<(String, String)>,
}

/// HTTP response packet sent from client to server
/// Note: Body is streamed separately via Data packets followed by End packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponsePacket {
    /// Matching stream_id from the request
    pub stream_id: String,

    /// HTTP status code
    pub status: u16,

    /// Response headers
    pub headers: Vec<(String, String)>,
}

impl HttpRequestPacket {
    /// Check if this is a WebSocket upgrade request
    pub fn is_websocket_upgrade(&self) -> bool {
        let has_upgrade_connection = self.headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("connection") && v.to_lowercase().contains("upgrade")
        });
        let has_websocket_upgrade = self.headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("upgrade") && v.eq_ignore_ascii_case("websocket")
        });
        has_upgrade_connection && has_websocket_upgrade
    }
}

impl HttpResponsePacket {
    /// Check if this is a WebSocket upgrade response (101 Switching Protocols)
    pub fn is_websocket_upgrade(&self) -> bool {
        self.status == 101
    }
}

impl ControlPacket {
    /// Serialize the packet to MessagePack bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        Ok(rmp_serde::to_vec(self)?)
    }

    /// Deserialize from MessagePack bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProtocolError> {
        Ok(rmp_serde::from_slice(data)?)
    }
}

/// Generate a new stream ID
pub fn new_stream_id() -> String {
    Uuid::new_v4().to_string()
}

/// Route information stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    /// IP address of the node hosting this tunnel
    pub node_ip: String,

    /// Internal port for node-to-node communication
    pub internal_port: u16,

    /// User ID for authorization checks
    pub user_id: String,
}

impl RouteInfo {
    pub fn new(node_ip: String, internal_port: u16, user_id: String) -> Self {
        Self {
            node_ip,
            internal_port,
            user_id,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Constants for the protocol
pub mod constants {
    /// Redis key prefix for routes
    pub const ROUTE_PREFIX: &str = "route:";

    /// Redis key prefix for usage tracking
    pub const USAGE_PREFIX: &str = "usage:";

    /// Redis key prefix for node registration
    pub const NODE_PREFIX: &str = "node";

    /// TTL for node registration (seconds) - nodes must heartbeat within this time
    pub const NODE_TTL_SECONDS: u64 = 60;

    /// TTL for route keys in seconds
    pub const ROUTE_TTL_SECONDS: u64 = 60;

    /// Heartbeat interval in seconds
    pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 30;

    /// Internal header for cluster authentication
    pub const CLUSTER_SECRET_HEADER: &str = "X-Cluster-Secret";

    /// Internal header for original host
    pub const ORIGINAL_HOST_HEADER: &str = "X-Original-Host";

    /// Header for subdomain override (local development)
    pub const SUBDOMAIN_HEADER: &str = "X-Subdomain";

    /// WebSocket ping interval
    pub const WS_PING_INTERVAL_SECONDS: u64 = 15;

    /// Protocol version - bumped for streaming support
    pub const PROTOCOL_VERSION: &str = "2.0.0";

    /// Bandwidth limits (bytes per month)
    pub const BANDWIDTH_FREE: u64 = 1 * 1024 * 1024 * 1024; // 1 GB
    pub const BANDWIDTH_HOBBY: u64 = 50 * 1024 * 1024 * 1024; // 50 GB
    pub const BANDWIDTH_PRO: u64 = 500 * 1024 * 1024 * 1024; // 500 GB

    /// Concurrent tunnel limits
    pub const CONCURRENT_TUNNELS_FREE: u32 = 5;
    pub const CONCURRENT_TUNNELS_HOBBY: u32 = 10;
    pub const CONCURRENT_TUNNELS_PRO: u32 = 50;

    /// Redis key prefix for user tunnel count
    pub const USER_TUNNELS_PREFIX: &str = "user_tunnels:";

    /// TTL for user tunnel count (seconds) - short TTL ensures stale counts auto-expire
    /// Heartbeat (30s) keeps it alive; if tunnel dies without cleanup, expires in ~1 min
    pub const USER_TUNNELS_TTL_SECONDS: i64 = 60;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_packet_roundtrip() {
        let packet = ControlPacket::Init(ClientHello {
            token: "test-token".to_string(),
            requested_subdomain: Some("my-app".to_string()),
            tunnel_type: TunnelType::Http,
            client_version: "0.1.0".to_string(),
        });

        let bytes = packet.to_bytes().unwrap();
        let decoded = ControlPacket::from_bytes(&bytes).unwrap();

        match decoded {
            ControlPacket::Init(hello) => {
                assert_eq!(hello.token, "test-token");
                assert_eq!(hello.requested_subdomain, Some("my-app".to_string()));
            }
            _ => panic!("Wrong packet type"),
        }
    }

    #[test]
    fn test_route_info_json() {
        let route = RouteInfo::new("192.168.1.1".to_string(), 6000, "user-123".to_string());

        let json = route.to_json().unwrap();
        let decoded = RouteInfo::from_json(&json).unwrap();

        assert_eq!(decoded.node_ip, "192.168.1.1");
        assert_eq!(decoded.internal_port, 6000);
        assert_eq!(decoded.user_id, "user-123");
    }

    #[test]
    fn test_http_request_packet() {
        let packet = ControlPacket::HttpRequest(HttpRequestPacket {
            stream_id: new_stream_id(),
            method: "POST".to_string(),
            uri: "/api/data?foo=bar".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Authorization".to_string(), "Bearer token".to_string()),
            ],
        });

        let bytes = packet.to_bytes().unwrap();
        let decoded = ControlPacket::from_bytes(&bytes).unwrap();

        match decoded {
            ControlPacket::HttpRequest(req) => {
                assert_eq!(req.method, "POST");
                assert_eq!(req.uri, "/api/data?foo=bar");
                assert_eq!(req.headers.len(), 2);
            }
            _ => panic!("Wrong packet type"),
        }
    }

    #[test]
    fn test_websocket_upgrade_detection() {
        let upgrade_request = HttpRequestPacket {
            stream_id: new_stream_id(),
            method: "GET".to_string(),
            uri: "/_next/webpack-hmr".to_string(),
            headers: vec![
                ("Connection".to_string(), "Upgrade".to_string()),
                ("Upgrade".to_string(), "websocket".to_string()),
                ("Sec-WebSocket-Key".to_string(), "dGhlIHNhbXBsZSBub25jZQ==".to_string()),
            ],
        };
        assert!(upgrade_request.is_websocket_upgrade());

        let normal_request = HttpRequestPacket {
            stream_id: new_stream_id(),
            method: "GET".to_string(),
            uri: "/api/data".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
        };
        assert!(!normal_request.is_websocket_upgrade());
    }
}
