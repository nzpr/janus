use super::ProtocolSpec;

pub const CAPABILITY: &str = "git_ssh";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[22],
    connect_fallback: true,
};
