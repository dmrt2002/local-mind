use local_mind::db::sqlite;

#[tokio::main]
async fn main() {
    env_logger::init();

    println!("Starting to regenerate all snippet summaries...");

    // Initialize database
    if let Err(e) = sqlite::init_database().await {
        eprintln!("Failed to initialize database: {}", e);
        std::process::exit(1);
    }

    // Regenerate all summaries
    match sqlite::regenerate_all_summaries().await {
        Ok(count) => {
            println!("Successfully regenerated {} summaries!", count);
        }
        Err(e) => {
            eprintln!("Failed to regenerate summaries: {}", e);
            std::process::exit(1);
        }
    }
}
