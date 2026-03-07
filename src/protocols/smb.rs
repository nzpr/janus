use super::ProtocolSpec;

pub const CAPABILITY: &str = "smb";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[445],
    connect_fallback: true,
};
