use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MODEL: &str = "llama3.1:8b";
const OLLAMA_BASE: &str = "http://127.0.0.1:11434";
const CHAT_TIMEOUT: Duration = Duration::from_secs(300);
const KEEP_ALIVE: &str = "30m";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: Some(content.into()), tool_calls: None, tool_name: None }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: Some(content.into()), tool_calls: None, tool_name: None }
    }

    pub fn tool(tool_name: &str, content: impl Into<String>) -> Self {
        Self { role: "tool".into(), content: Some(content.into()), tool_calls: None, tool_name: Some(tool_name.into()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

impl FunctionCall {
    /// Ollama normally returns `arguments` as a JSON object, but smaller models
    /// sometimes emit a JSON string instead. Normalize both to an object.
    pub fn arguments_object(&self) -> Result<Value, String> {
        match &self.arguments {
            value @ Value::Object(_) => Ok(value.clone()),
            Value::String(raw) => serde_json::from_str(raw)
                .map_err(|e| format!("tool '{}' arguments are not valid JSON: {e}", self.name)),
            Value::Null => Ok(serde_json::json!({})),
            _ => Err(format!("tool '{}' arguments must be a JSON object", self.name)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OllamaClient {
    http: reqwest::Client,
    base: String,
    model: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    tools: &'a [Value],
    stream: bool,
    keep_alive: &'a str,
    options: Value,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Deserialize)]
struct TagEntry {
    name: String,
}

impl OllamaClient {
    pub fn new() -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(CHAT_TIMEOUT)
            .build()
            .map_err(|e| format!("could not create HTTP client: {e}"))?;
        let model = std::env::var("EPICORGANIZER_MODEL")
            .ok()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Ok(Self { http, base: OLLAMA_BASE.to_string(), model })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub async fn chat(&self, messages: &[ChatMessage], tools: &[Value], num_predict: u32) -> Result<ChatMessage, String> {
        let request = ChatRequest {
            model: &self.model,
            messages,
            tools,
            stream: false,
            keep_alive: KEEP_ALIVE,
            options: serde_json::json!({ "temperature": 0, "num_ctx": 4096, "num_predict": num_predict }),
        };
        let response = self
            .http
            .post(format!("{}/api/chat", self.base))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama is not reachable at {} — is `ollama serve` running? ({e})", self.base))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Ollama error {status}: {body}"));
        }
        let parsed: ChatResponse = response
            .json()
            .await
            .map_err(|e| format!("unexpected Ollama response: {e}"))?;
        Ok(parsed.message)
    }

    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        let response = self
            .http
            .get(format!("{}/api/tags", self.base))
            .send()
            .await
            .map_err(|e| format!("Ollama is not reachable at {}: {e}", self.base))?;
        if !response.status().is_success() {
            return Err(format!("Ollama error {}", response.status()));
        }
        let tags: TagsResponse = response
            .json()
            .await
            .map_err(|e| format!("unexpected Ollama response: {e}"))?;
        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// Load the model into memory and keep it resident so the first planning
    /// call does not pay the cold-load penalty.
    pub async fn warm_up(&self) -> Result<(), String> {
        let payload = serde_json::json!({
            "model": self.model,
            "prompt": "",
            "stream": false,
            "keep_alive": KEEP_ALIVE,
        });
        let response = self
            .http
            .post(format!("{}/api/generate", self.base))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("model warm-up failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("model warm-up failed: {}", response.status()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_object_arguments() {
        let call = FunctionCall { name: "move_file".into(), arguments: serde_json::json!({ "source": "a" }) };
        assert_eq!(call.arguments_object().unwrap()["source"], "a");
    }

    #[test]
    fn normalizes_string_arguments() {
        let call = FunctionCall { name: "move_file".into(), arguments: Value::String("{\"source\":\"a\"}".into()) };
        assert_eq!(call.arguments_object().unwrap()["source"], "a");
    }

    #[test]
    fn rejects_invalid_string_arguments() {
        let call = FunctionCall { name: "move_file".into(), arguments: Value::String("not json".into()) };
        assert!(call.arguments_object().is_err());
    }
}
