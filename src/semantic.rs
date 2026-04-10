use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self, String> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )
        .map_err(|e| format!("failed to load embedding model: {}", e))?;
        Ok(Embedder { model })
    }

    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut results = self
            .model
            .embed(vec![text], None)
            .map_err(|e| format!("embedding failed: {}", e))?;
        results
            .pop()
            .ok_or_else(|| "no embedding returned".to_string())
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        self.model
            .embed(texts.to_vec(), None)
            .map_err(|e| format!("batch embedding failed: {}", e))
    }
}

/// Check if the model is cached locally (fastembed caches in ~/.cache/huggingface/).
pub fn model_is_cached() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        // AllMiniLML6V2 model directory
        let cache_path = std::path::PathBuf::from(home)
            .join(".cache")
            .join("huggingface")
            .join("hub");
        if cache_path.exists() {
            // Check if there's any model directory that looks like our model
            if let Ok(entries) = std::fs::read_dir(&cache_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.contains("all-MiniLM") || name_str.contains("all_minilm") {
                        return true;
                    }
                }
            }
            // Also check if the fastembed-specific cache directory exists with content
            let fastembed_cache = std::path::PathBuf::from(
                std::env::var("HOME").unwrap_or_default(),
            )
            .join(".cache")
            .join("huggingface");
            if fastembed_cache.exists() {
                // Check for onnx model files that fastembed downloads
                if let Ok(entries) = std::fs::read_dir(&fastembed_cache) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.contains("minilm") || name_str.contains("MiniLM") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    } else {
        false
    }
}
