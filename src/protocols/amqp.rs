use super::ProtocolSpec;

pub const CAPABILITY: &str = "amqp";
pub const SPEC: ProtocolSpec = ProtocolSpec {
    capability: CAPABILITY,
    ports: &[5672],
    connect_fallback: true,
};
