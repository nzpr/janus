use super::ProtocolSpec;

pub const CAPABILITY: &str = "postgres_wire";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[5432],
    connect_fallback: true,
};
