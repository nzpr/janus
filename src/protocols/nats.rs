use super::ProtocolSpec;

pub const CAPABILITY: &str = "nats";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[4222],
    connect_fallback: true,
};
