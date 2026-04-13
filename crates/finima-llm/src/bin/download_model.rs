//! Pre-download the default GGUF model for Candle inference.
//!
//! Run with: `cargo run -p finima-llm --features candle --bin download_model`
//!
//! This downloads the model to the HuggingFace Hub cache directory
//! (~/.cache/huggingface/hub/) so that application startup does not
//! incur a cold download.

fn main() {
    // Initialise minimal tracing so download progress is visible.
    tracing_subscriber::fmt().with_env_filter("info").init();

    match finima_llm::model_download::download_default_model() {
        Ok(path) => {
            println!("Model ready at: {}", path.display());
        }
        Err(e) => {
            eprintln!("Download failed: {e}");
            std::process::exit(1);
        }
    }
}
