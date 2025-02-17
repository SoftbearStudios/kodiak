use crate::bitcode::{self, *};
use crate::is_default;
use serde::{Deserialize, Serialize};

/// Navigation performance measured by client.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct NavigationMetricsDto {
    /// DNS query latency.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dns: u16,
    /// TCP establishment latency, not counting TLS (if any).
    #[serde(default, skip_serializing_if = "is_default")]
    pub tcp: u16,
    /// TLS establishment latency.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tls: u16,
    /// HTTP request and response latency total.
    #[serde(default, skip_serializing_if = "is_default")]
    pub http: u16,
    /// DOM loading latency, after HTTP response.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dom: u16,
}
