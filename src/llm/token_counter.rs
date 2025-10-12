use reqwest::{blocking::Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
pub struct Part {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

impl Content {
    pub fn from_text(role: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            parts: vec![Part { text: text.into() }],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CountTokensRequest {
    contents: Vec<Content>,
}

#[derive(Debug, Deserialize)]
struct CountTokensResponse {
    #[serde(rename = "totalTokens")]
    total_tokens: Option<i64>,
    #[serde(rename = "cachedContentTokenCount")]
    cached_content_token_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TokenCountSummary {
    pub total_tokens: i64,
    pub cached_content_token_count: Option<i64>,
}

#[derive(Debug, Error)]
pub enum TokenCountError {
    #[error("Token counting requires at least one content part")]
    EmptyRequest,
    #[error("Token counting request failed: {0}")]
    Request(reqwest::Error),
    #[error("Token counting response error ({status}): {message}")]
    Response { status: StatusCode, message: String },
    #[error("Failed to parse token counting response: {0}")]
    Parse(reqwest::Error),
    #[error("Token counting response missing totalTokens field")]
    MissingTotalTokens,
}

pub fn count_tokens(
    api_key: &str,
    model: &str,
    contents: Vec<Content>,
) -> Result<TokenCountSummary, TokenCountError> {
    if contents.is_empty() {
        return Err(TokenCountError::EmptyRequest);
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/{model}:countTokens");

    let client = Client::new();
    let request = CountTokensRequest { contents };

    let response = client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&request)
        .send()
        .map_err(TokenCountError::Request)?;

    let response = response
        .error_for_status()
        .map_err(|err| TokenCountError::Response {
            status: err.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message: err.to_string(),
        })?;

    let parsed: CountTokensResponse = response.json().map_err(TokenCountError::Parse)?;
    let total_tokens = parsed
        .total_tokens
        .ok_or(TokenCountError::MissingTotalTokens)?;

    Ok(TokenCountSummary {
        total_tokens,
        cached_content_token_count: parsed.cached_content_token_count,
    })
}
