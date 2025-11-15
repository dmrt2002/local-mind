// Llama.cpp integration for local LLM inference
use anyhow::{Context, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel as LlamaCppModel, Special};
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Global backend initialization (must be initialized once)
static BACKEND_INIT: std::sync::Once = std::sync::Once::new();

fn init_backend() {
    BACKEND_INIT.call_once(|| {
        LlamaBackend::init().expect("Failed to initialize llama backend");
    });
}

/// Wrapper around llama-cpp model for easier use
pub struct LlamaModel {
    model: Arc<LlamaCppModel>,
    backend: Arc<Mutex<LlamaBackend>>,
}

/// Parameters for LLM inference
pub struct LlamaParams {
    pub n_threads: usize,
    pub n_ctx: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub repeat_penalty: f32,
    pub max_tokens: usize,
}

impl Default for LlamaParams {
    fn default() -> Self {
        Self {
            n_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            n_ctx: 4096,          // Increased from 1024 - needed for categorization prompts (~1500 tokens)
            temperature: 0.3,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            max_tokens: 128,      // Reduced from 200 - JSON responses are typically 50-100 tokens
        }
    }
}

impl LlamaModel {
    /// Load model from file path
    pub fn new(model_path: &str) -> Result<Self> {
        let path = Path::new(model_path);

        if !path.exists() {
            anyhow::bail!("Model file not found: {}", model_path);
        }

        // Initialize backend
        init_backend();
        let backend = LlamaBackend::init().context("Failed to initialize backend")?;

        log::info!("Loading LLM model from: {}", model_path);

        // Set up model parameters
        let model_params = LlamaModelParams::default();

        // Load the model
        let model = LlamaCppModel::load_from_file(&backend, model_path, &model_params)
            .context("Failed to load model file")?;

        log::info!("✅ LLM model loaded successfully");

        Ok(Self {
            model: Arc::new(model),
            backend: Arc::new(Mutex::new(backend)),
        })
    }

    /// Load model with progress callback
    pub fn load_with_callback<F>(model_path: &str, mut callback: F) -> Result<Self>
    where
        F: FnMut(usize),
    {
        callback(0);
        let model = Self::new(model_path)?;
        callback(100);
        Ok(model)
    }

    /// Generate text completion
    pub fn generate(&self, prompt: &str, params: &LlamaParams) -> Result<String> {
        log::debug!("Generating LLM response for prompt (length: {})", prompt.len());

        // Create context parameters
        // Note: n_batch defaults to 512 in llama-cpp-2, but we need it to be at least as large as our prompt
        // The batch size should be >= number of input tokens
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(params.n_ctx as u32))
            .with_n_threads(params.n_threads as i32)
            .with_n_threads_batch(params.n_threads as i32);
        
        // Note: llama-cpp-2 doesn't expose n_batch directly in LlamaContextParams
        // The default n_batch is typically 512, which should be sufficient for most prompts
        // If prompts exceed this, we'll get the assertion error and need to reduce prompt size

        // Create context
        let mut ctx = self.model
            .new_context(&*self.backend.lock().unwrap(), ctx_params)
            .context("Failed to create context")?;

        // Tokenize the prompt
        let tokens = self.model
            .str_to_token(prompt, AddBos::Always)
            .context("Failed to tokenize prompt")?;

        log::info!("Tokenized prompt: {} tokens", tokens.len());
        log::info!("Prompt length: {} characters, {} tokens (max context: {} tokens)",
            prompt.len(), tokens.len(), params.n_ctx);
        
        // Check if prompt might exceed default batch size (typically 512)
        // llama-cpp-2 default n_batch is 512, so we warn if we're close
        if tokens.len() > 400 {
            log::warn!("⚠️  Prompt is large ({} tokens) - may exceed default batch size (512). If you see batch size errors, reduce prompt length.", tokens.len());
        }

        if tokens.len() >= params.n_ctx {
            log::error!("❌ CRITICAL: Prompt exceeds context window!");
            log::error!("   Prompt: {} characters", prompt.len());
            log::error!("   Tokens: {} (max: {})", tokens.len(), params.n_ctx);
            log::error!("   Ratio: {:.2} chars/token", prompt.len() as f32 / tokens.len() as f32);
            log::error!("   First 200 chars: {}", &prompt[..200.min(prompt.len())]);
            anyhow::bail!(
                "Prompt too long: {} tokens (max: {})",
                tokens.len(),
                params.n_ctx
            );
        }

        // Create batch for input tokens
        let mut batch = LlamaBatch::new(params.n_ctx, 1);

        // Add tokens to batch
        for (i, token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch.add(*token, i as i32, &[0], is_last)
                .context("Failed to add token to batch")?;
        }

        // Decode the batch
        ctx.decode(&mut batch)
            .context("Failed to decode batch")?;

        // Generate tokens
        let mut output_tokens = Vec::new();
        let mut generated_text = String::new();

        for _ in 0..params.max_tokens {
            // Get logits for the last token
            let candidates = ctx.candidates_ith(batch.n_tokens() - 1);

            let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);

            // Apply sampling
            // Note: llama-cpp-2 has limited sampling support
            // For production, consider using a more advanced sampling library
            let new_token_id = if params.temperature <= 0.0 {
                // Greedy sampling - just take the most likely token
                candidates_p.data[0].id()
            } else {
                // Simple random sampling with seed
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u32;
                candidates_p.sample_token(seed)
            };

            // Check for EOS token
            if self.model.is_eog_token(new_token_id) {
                break;
            }

            // Convert token to text
            let piece = self.model
                .token_to_str(new_token_id, Special::Tokenize)
                .context("Failed to convert token to string")?;

            generated_text.push_str(&piece);
            output_tokens.push(new_token_id);

            // Prepare for next iteration
            batch.clear();
            batch.add(new_token_id, tokens.len() as i32 + output_tokens.len() as i32 - 1, &[0], true)
                .context("Failed to add new token to batch")?;

            // Decode
            ctx.decode(&mut batch)
                .context("Failed to decode new token")?;
        }

        log::debug!("Generated {} tokens", output_tokens.len());
        log::debug!("Generated text: {}", generated_text);

        Ok(generated_text)
    }

    /// Get context size
    pub fn n_ctx(&self) -> usize {
        2048
    }
}

// Thread-safe clone
impl Clone for LlamaModel {
    fn clone(&self) -> Self {
        Self {
            model: Arc::clone(&self.model),
            backend: Arc::clone(&self.backend),
        }
    }
}
