// Global LLM manager singleton for use across the application
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

use crate::inference::llm_manager::LlmManager;



/// Global LLM manager instance
static GLOBAL_LLM_MANAGER: OnceCell<Arc<RwLock<Option<LlmManager>>>> = OnceCell::new();

/// Initialize the global LLM manager
pub fn init_global_llm(manager: LlmManager) {
    GLOBAL_LLM_MANAGER.get_or_init(|| Arc::new(RwLock::new(Some(manager))));
}

/// Get the global LLM manager
pub fn get_global_llm() -> Option<LlmManager> {
    GLOBAL_LLM_MANAGER
        .get()
        .and_then(|arc| arc.read().clone())
}

/// Check if global LLM is initialized
pub fn is_global_llm_initialized() -> bool {
    GLOBAL_LLM_MANAGER.get().is_some()
}
