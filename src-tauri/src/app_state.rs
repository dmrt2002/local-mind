use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppState {
    FullyOperational {
        fts5: ComponentState,
        embedding: ComponentState,
        llm: ComponentState,
    },
    SearchOnly {
        fts5: ComponentState,
        embedding: ComponentState,
        llm: ComponentState,
    },
    KeywordOnly {
        fts5: ComponentState,
        embedding: ComponentState,
        llm: ComponentState,
    },
    Degraded {
        fts5: ComponentState,
        embedding: ComponentState,
        llm: ComponentState,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentState {
    Ready,
    NotInstalled,
    Failed(String),
    Loading,
}

impl AppState {
    pub fn new() -> Self {
        // Start with all components as Ready
        // Will be updated based on actual initialization results
        Self::FullyOperational {
            fts5: ComponentState::Ready,
            embedding: ComponentState::Ready,
            llm: ComponentState::Ready,
        }
    }

    pub async fn detect_initial_state() -> Self {
        // Check if each component is available
        let fts5 = ComponentState::Ready; // SQLite is always available
        
        let embedding = if check_embedding_available().await {
            ComponentState::Ready
        } else {
            ComponentState::NotInstalled
        };

        let llm = if check_llm_available().await {
            ComponentState::Ready
        } else {
            ComponentState::NotInstalled
        };

        match (embedding.clone(), llm.clone()) {
            (ComponentState::Ready, ComponentState::Ready) => {
                Self::FullyOperational { fts5, embedding, llm }
            }
            (ComponentState::Ready, _) => {
                Self::Degraded { fts5, embedding, llm }
            }
            (_, _) => {
                Self::KeywordOnly { fts5, embedding, llm }
            }
        }
    }

    pub fn show_banner(&self) -> Option<String> {
        match self {
            Self::FullyOperational { .. } => None,
            Self::SearchOnly { fts5: _, embedding, llm } => {
                Some(format!(
                    "⚠️ AI features unavailable: {} | {}",
                    format_state(embedding),
                    format_state(llm)
                ))
            }
            Self::KeywordOnly { .. } => {
                Some("⚠️ Running in keyword-only mode. Semantic search and AI features unavailable.".to_string())
            }
            Self::Degraded { llm, .. } => {
                Some(format!(
                    "⚠️ AI Q&A unavailable: {}",
                    format_state(llm)
                ))
            }
        }
    }

    pub fn can_search_semantic(&self) -> bool {
        matches!(
            self,
            Self::FullyOperational { embedding: ComponentState::Ready, .. } |
            Self::Degraded { embedding: ComponentState::Ready, .. }
        )
    }

    pub fn can_ask_ai(&self) -> bool {
        matches!(
            self,
            Self::FullyOperational { llm: ComponentState::Ready, .. }
        )
    }
}

fn format_state(state: &ComponentState) -> String {
    match state {
        ComponentState::Ready => "Ready".to_string(),
        ComponentState::NotInstalled => "Not Installed".to_string(),
        ComponentState::Failed(msg) => format!("Failed: {}", msg),
        ComponentState::Loading => "Loading...".to_string(),
    }
}

async fn check_embedding_available() -> bool {
    // Check if fastembed model can be loaded
    // This is a placeholder - actual implementation will check model files
    true
}

async fn check_llm_available() -> bool {
    // Check if GGUF model file exists
    use crate::inference::llm_manager;

    match llm_manager::get_default_model_path() {
        Ok(path) => {
            let exists = path.exists();
            if !exists {
                log::warn!("LLM model not found at: {:?}", path);
            }
            exists
        }
        Err(e) => {
            log::warn!("Failed to get LLM model path: {}", e);
            false
        }
    }
}
