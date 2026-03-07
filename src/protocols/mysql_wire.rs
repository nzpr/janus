use super::ProtocolSpec;

pub const CAPABILITY: &str = "mysql_wire";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[3306],
    connect_fallback: true,
};
