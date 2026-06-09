mod backend;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    backend::run().await
}
