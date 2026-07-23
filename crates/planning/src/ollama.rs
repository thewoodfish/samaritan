//! An [`Model`] backed by a local Ollama server (`http://localhost:11434`).
//!
//! Calls `/api/chat` with `format: "json"` and `temperature: 0` for greedy,
//! reproducible decoding. Synchronous — planning is single-threaded.

use serde_json::json;

use crate::model::{Model, ModelError};

/// A handle to a local Ollama model, e.g. `OllamaModel::new("llama3.1")`.
#[derive(Debug, Clone)]
pub struct OllamaModel {
    model: String,
    endpoint: String,
}

impl OllamaModel {
    /// Use `model` on the default local endpoint.
    pub fn new(model: impl Into<String>) -> OllamaModel {
        OllamaModel {
            model: model.into(),
            endpoint: "http://localhost:11434".to_owned(),
        }
    }

    /// Override the endpoint (e.g. a remote Ollama host).
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

impl Model for OllamaModel {
    fn id(&self) -> &str {
        &self.model
    }

    fn complete_json(
        &self,
        _stage: &str,
        system: &str,
        user: &str,
    ) -> Result<serde_json::Value, ModelError> {
        let body = json!({
            "model": self.model,
            "stream": false,
            "format": "json",
            "options": { "temperature": 0, "seed": 0 },
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });

        let resp = ureq::post(&format!("{}/api/chat", self.endpoint))
            .send_json(body)
            .map_err(|e| ModelError::Transport(e.to_string()))?;
        let value: serde_json::Value = resp
            .into_json()
            .map_err(|e| ModelError::Transport(e.to_string()))?;

        let content = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| ModelError::MissingField("message.content".to_owned()))?;

        serde_json::from_str(content).map_err(|e| ModelError::BadJson(e.to_string()))
    }
}
