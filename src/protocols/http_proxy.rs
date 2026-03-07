use super::ProtocolSpec;

pub const CAPABILITY: &str = "http_proxy";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[],
    connect_fallback: false,
};
