mod mcp;
mod protocols;

fn main() -> anyhow::Result<()> {
    mcp::run()
}
