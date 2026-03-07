use super::ProtocolSpec;

pub const CAPABILITY: &str = "ldap";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[389, 636],
    connect_fallback: true,
};
