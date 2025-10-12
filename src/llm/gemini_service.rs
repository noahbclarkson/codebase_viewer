use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, CreateChatCompletionResponse,
    },
    Client,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Gemini API error: {0}")]
    OpenAI(#[from] async_openai::error::OpenAIError),
    #[error("Gemini configuration error: {0}")]
    Config(String),
    #[error("Gemini internal error: {0}")]
    Internal(String),
}

fn create_gemini_client(api_key: &str) -> Client<OpenAIConfig> {
    let api_base = "https://generativelanguage.googleapis.com/v1beta/openai".to_string();
    let config = OpenAIConfig::new()
        .with_api_base(api_base)
        .with_api_key(api_key.to_string());
    Client::with_config(config)
}

pub async fn query_codebase(
    api_key: &str,
    model: &str,
    context: String,
    query: String,
    temperature: f32,
) -> Result<String, AppError> {
    if api_key.trim().is_empty() {
        return Err(AppError::Config("Gemini API key is empty".into()));
    }

    let client = create_gemini_client(api_key);

    let system_message = ChatCompletionRequestSystemMessageArgs::default()
        .content(format!(
            "You are an expert software development assistant. Analyze the following codebase report and answer the user's query based only on the provided information.\n\n--- CODEBASE REPORT ---\n\n{context}"
        ))
        .build()?;

    let user_message = ChatCompletionRequestUserMessageArgs::default()
        .content(query)
        .build()?;

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .temperature(temperature)
        .messages([system_message.into(), user_message.into()])
        .build()?;

    let response: CreateChatCompletionResponse = client.chat().create(request).await?;

    if let Some(choice) = response.choices.first() {
        if let Some(content) = &choice.message.content {
            Ok(content.clone())
        } else {
            Err(AppError::Internal(
                "Gemini response did not include any text".into(),
            ))
        }
    } else {
        Err(AppError::Internal(
            "Gemini response did not include any choices".into(),
        ))
    }
}
