use once_cell::sync::Lazy;
use thiserror::Error;
use tiktoken_rs::cl100k_base;

#[derive(Debug, Clone)]
pub struct TokenCountSummary {
    pub total_tokens: i64,
    pub cached_content_token_count: Option<i64>,
}

#[derive(Debug, Error)]
pub enum TokenCountError {
    #[error("Tokenizer unavailable: {0}")]
    TokenizerUnavailable(String),
}

static BPE: Lazy<Option<tiktoken_rs::CoreBPE>> = Lazy::new(|| match cl100k_base() {
    Ok(bpe) => Some(bpe),
    Err(err) => {
        log::warn!("Failed to load cl100k_base tokenizer: {err}");
        None
    }
});

/// Counts tokens using the cl100k_base encoding (used by GPT-4 and Gemini approximations).
/// Falls back to a whitespace count if the tokenizer can't load.
pub fn count_tokens_local(text: &str) -> usize {
    if let Some(bpe) = BPE.as_ref() {
        bpe.encode_with_special_tokens(text).len()
    } else {
        text.split_whitespace().count()
    }
}

pub fn count_tokens(text: &str) -> Result<TokenCountSummary, TokenCountError> {
    Ok(TokenCountSummary {
        total_tokens: count_tokens_local(text) as i64,
        cached_content_token_count: None,
    })
}
