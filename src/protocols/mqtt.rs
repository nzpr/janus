use super::ProtocolSpec;

pub const CAPABILITY: &str = "mqtt";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[1883, 8883],
    connect_fallback: true,
};
