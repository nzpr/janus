mod janusd;
mod protocols;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    janusd::run().await
}
