#[derive(Clone, Copy)]
pub struct ProtocolSpec {
    pub capability: &'static str,
    pub ports: &'static [u16],
    #[allow(dead_code)]
    pub connect_fallback: bool,
}

pub mod amqp;
pub mod git_http;
pub mod git_ssh;
pub mod http_proxy;
pub mod kafka;
pub mod ldap;
pub mod mongodb;
pub mod mqtt;
pub mod mysql_wire;
pub mod nats;
pub mod postgres_wire;
pub mod redis;
pub mod sftp;
pub mod smb;

const ALL_PROTOCOLS: [ProtocolSpec; 14] = [
    http_proxy::SPEC,
    git_http::SPEC,
    git_ssh::SPEC,
    postgres_wire::SPEC,
    mysql_wire::SPEC,
    redis::SPEC,
    mongodb::SPEC,
    amqp::SPEC,
    kafka::SPEC,
    nats::SPEC,
    mqtt::SPEC,
    ldap::SPEC,
    sftp::SPEC,
    smb::SPEC,
];

#[allow(dead_code)]
pub fn all() -> &'static [ProtocolSpec] {
    &ALL_PROTOCOLS
}

#[allow(dead_code)]
pub fn proxy_capabilities() -> Vec<&'static str> {
    ALL_PROTOCOLS.iter().map(|spec| spec.capability).collect()
}

#[allow(dead_code)]
pub fn connect_capabilities_for_port(port: u16) -> Vec<&'static str> {
    ALL_PROTOCOLS
        .iter()
        .filter(|spec| spec.connect_fallback && spec.ports.contains(&port))
        .map(|spec| spec.capability)
        .collect()
}
