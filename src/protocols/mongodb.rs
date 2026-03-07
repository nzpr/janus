use super::ProtocolSpec;

pub const CAPABILITY: &str = "mongodb";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[27017],
    connect_fallback: true,
};
