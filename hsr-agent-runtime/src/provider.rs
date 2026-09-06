use crate::{ModelConfig, TokenUsage};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fmt, time::Duration};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub raw_arguments: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMessage {
    System(String),
    User(String),
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ModelToolCall>,
    },
    Tool {
        call_id: String,
        name: String,
        content: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub require_tool: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelResponse {
    pub response_id: String,
    pub content: Option<String>,
    pub tool_calls: Vec<ModelToolCall>,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    InvalidEndpoint,
    RequestFailed(String),
    HttpStatus { status: u16, message: String },
    InvalidResponse(String),
    MissingUsage,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint => f.write_str("API Endpoint 无法组成 chat/completions 地址"),
            Self::RequestFailed(message) => write!(f, "模型请求失败：{message}"),
            Self::HttpStatus { status, message } => {
                write!(f, "模型服务返回 HTTP {status}：{message}")
            }
            Self::InvalidResponse(message) => write!(f, "模型响应格式无效：{message}"),
            Self::MissingUsage => {
                f.write_str("模型响应没有 usage，无法精确统计 Token；本次结果不接受")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

pub trait ModelProvider: Send + Sync {
    fn complete(
        &self,
        config: &ModelConfig,
        request: &ProviderRequest,
    ) -> Result<ModelResponse, ProviderError>;
}

#[derive(Clone)]
pub struct OpenAiCompatibleProvider {
    client: Client,
}

impl Default for OpenAiCompatibleProvider {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .expect("HTTP client"),
        }
    }
}

impl OpenAiCompatibleProvider {
    fn chat_url(endpoint: &str) -> Result<String, ProviderError> {
        let endpoint = endpoint.trim().trim_end_matches('/');
        if endpoint.is_empty() {
            return Err(ProviderError::InvalidEndpoint);
        }
        if endpoint.ends_with("/chat/completions") {
            Ok(endpoint.into())
        } else {
            Ok(format!("{endpoint}/chat/completions"))
        }
    }
}

impl ModelProvider for OpenAiCompatibleProvider {
    fn complete(
        &self,
        config: &ModelConfig,
        request: &ProviderRequest,
    ) -> Result<ModelResponse, ProviderError> {
        let messages: Vec<_> = request.messages.iter().map(message_json).collect();
        let tools: Vec<_> = request.tools.iter().map(|tool| json!({
            "type":"function", "function": {"name":tool.name,"description":tool.description,"parameters":tool.parameters}
        })).collect();
        let mut body = json!({
            "model": config.model(),
            "messages": messages,
            "tools": tools,
            "tool_choice": if request.require_tool { "required" } else { "auto" },
            "parallel_tool_calls": false,
            "max_completion_tokens": config.context_length(),
        });
        if let Some(effort) = config.reasoning_mode().as_api_value() {
            body["reasoning_effort"] = json!(effort);
        }
        let mut call = self
            .client
            .post(Self::chat_url(config.endpoint())?)
            .json(&body);
        if !config.api_key().is_empty() {
            call = call.bearer_auth(config.api_key());
        }
        let response = call.send().map_err(|error| {
            ProviderError::RequestFailed(redact_secret(error.to_string(), config.api_key()))
        })?;
        let status = response.status();
        let bytes = response
            .bytes()
            .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
        if !status.is_success() {
            let message = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(400).collect());
            return Err(ProviderError::HttpStatus {
                status: status.as_u16(),
                message: redact_secret(message, config.api_key()),
            });
        }
        let response: WireResponse = serde_json::from_slice(&bytes)
            .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
        let usage = response.usage.ok_or(ProviderError::MissingUsage)?;
        let message = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::InvalidResponse("choices 为空".into()))?
            .message;
        let tool_calls = message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|call| {
                if call.kind != "function" {
                    return Err(ProviderError::InvalidResponse(format!(
                        "不支持的 tool call 类型 {}",
                        call.kind
                    )));
                }
                let arguments = serde_json::from_str(&call.function.arguments)
                    .unwrap_or_else(|_| Value::String(call.function.arguments.clone()));
                Ok(ModelToolCall {
                    id: call.id,
                    name: call.function.name,
                    arguments,
                    raw_arguments: call.function.arguments,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ModelResponse {
            response_id: response.id,
            content: message.content.filter(|v| !v.trim().is_empty()),
            tool_calls,
            usage: TokenUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            },
        })
    }
}

fn redact_secret(message: String, secret: &str) -> String {
    if secret.is_empty() {
        message
    } else {
        message.replace(secret, "[REDACTED]")
    }
}

fn message_json(message: &ChatMessage) -> Value {
    match message {
        ChatMessage::System(content) => json!({"role":"system","content":content}),
        ChatMessage::User(content) => json!({"role":"user","content":content}),
        ChatMessage::Assistant {
            content,
            tool_calls,
        } => json!({
            "role":"assistant", "content":content,
            "tool_calls":tool_calls.iter().map(|call| json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.raw_arguments}})).collect::<Vec<_>>()
        }),
        ChatMessage::Tool {
            call_id,
            name,
            content,
        } => {
            json!({"role":"tool","tool_call_id":call_id,"name":name,"content":content})
        }
    }
}

#[derive(Deserialize)]
struct WireResponse {
    id: String,
    choices: Vec<WireChoice>,
    usage: Option<WireUsage>,
}
#[derive(Deserialize)]
struct WireChoice {
    message: WireMessage,
}
#[derive(Deserialize)]
struct WireMessage {
    content: Option<String>,
    tool_calls: Option<Vec<WireToolCall>>,
}
#[derive(Deserialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: WireFunction,
}
#[derive(Deserialize)]
struct WireFunction {
    name: String,
    arguments: String,
}
#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelConfigPatch, ReasoningMode};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
    };

    #[test]
    fn endpoint_accepts_base_or_full_chat_path() {
        assert_eq!(
            OpenAiCompatibleProvider::chat_url("http://localhost:1/v1").unwrap(),
            "http://localhost:1/v1/chat/completions"
        );
        assert_eq!(
            OpenAiCompatibleProvider::chat_url("http://localhost:1/v1/chat/completions").unwrap(),
            "http://localhost:1/v1/chat/completions"
        );
    }

    #[test]
    fn provider_errors_redact_configured_key() {
        assert_eq!(
            redact_secret("provider echoed very-secret".into(), "very-secret"),
            "provider echoed [REDACTED]"
        );
    }

    #[test]
    fn real_http_boundary_sends_config_and_reads_exact_usage() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                received.extend_from_slice(&buffer[..count]);
                if let Some(header_end) = received.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&received[..header_end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if received.len() >= header_end + 4 + length {
                        break;
                    }
                }
            }
            sender.send(String::from_utf8(received).unwrap()).unwrap();
            let body =
                json!({"id":"wire-1","choices":[{"message":{"role":"assistant","content":"ok"}}],
                "usage":{"prompt_tokens":123,"completion_tokens":45,"total_tokens":168}})
                .to_string();
            write!(stream,"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let mut config = ModelConfig::default();
        config
            .apply(ModelConfigPatch {
                endpoint: format!("http://{address}/v1"),
                api_key: Some("very-secret".into()),
                clear_api_key: false,
                model: "test-model".into(),
                context_length: 2048,
                reasoning_mode: ReasoningMode::Low,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                token_budget: 1000,
            })
            .unwrap();
        let response = OpenAiCompatibleProvider::default()
            .complete(
                &config,
                &ProviderRequest {
                    messages: vec![ChatMessage::User("hello".into())],
                    tools: vec![],
                    require_tool: false,
                },
            )
            .unwrap();
        assert_eq!(
            response.usage,
            TokenUsage {
                input_tokens: 123,
                output_tokens: 45
            }
        );
        let request = receiver.recv().unwrap();
        assert!(
            request.contains("authorization: Bearer very-secret")
                || request.contains("Authorization: Bearer very-secret")
        );
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["reasoning_effort"], "low");
        assert_eq!(body["max_completion_tokens"], 2048);
    }
}
