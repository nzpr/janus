use super::ProtocolSpec;

pub const CAPABILITY: &str = "redis";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[6379],
    connect_fallback: true,
};
