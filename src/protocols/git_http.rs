use super::ProtocolSpec;

pub const CAPABILITY: &str = "git_http";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[],
    connect_fallback: false,
};
