mod pg_sidecar;
mod protocols;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pg_sidecar::run().await
}
