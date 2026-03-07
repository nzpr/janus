use super::ProtocolSpec;

pub const CAPABILITY: &str = "kafka";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[9092],
    connect_fallback: true,
};
