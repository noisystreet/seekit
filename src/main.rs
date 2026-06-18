#[tokio::main]
async fn main() {
    if let Err(e) = seekit::run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
