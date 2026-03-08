mod protocols;
mod tunnel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tunnel::run().await
}
