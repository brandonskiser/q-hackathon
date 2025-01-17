mod chat;
mod code;
mod system_prompts;
use chat::{execute_chat, ChatArgs};
use code::{execute_code, CodeArgs};
use system_prompts::SYSTEM_PROMPT;

use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::{
    error::SdkError, operation::converse::ConverseError, types::SystemContentBlock, Client,
};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::IsTerminal, path::Path, sync::Arc};
use thiserror::Error;
use tokio::{io::AsyncReadExt, sync::Mutex, time::Instant};

use clap::{Parser, Subcommand};
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
#[command(
    author = "Amazon Q",
    version,
    about = "An AI-powdered CLI for your terminal and editor."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Chat(ChatArgs),
    Code(CodeArgs),
}

#[derive(Serialize, Deserialize, Default, Debug)]
enum ModelResponseType {
    #[default]
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "code")]
    Code,
}

impl std::fmt::Display for ModelResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelResponseType::Chat => write!(f, "chat"),
            ModelResponseType::Code => write!(f, "code"),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct ModelResponse {
    #[serde(rename = "type")]
    type_: ModelResponseType,
    // #[serde(deserialize_with = "deserialize_with_escaped_newline")]
    message: Vec<String>,
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Clone, Debug)]
struct StorableMessage {
    role: String,
    content: String,
}

// This represents the Anthropic/Claude message format for Bedrock
#[derive(Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Serialize, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

impl From<&StorableMessage> for AnthropicMessage {
    fn from(msg: &StorableMessage) -> Self {
        AnthropicMessage {
            role: msg.role.clone(),
            content: vec![AnthropicContent {
                content_type: "text".to_string(),
                text: msg.content.clone(),
            }],
        }
    }
}

impl From<AnthropicMessage> for StorableMessage {
    fn from(msg: AnthropicMessage) -> Self {
        let content = msg
            .content
            .into_iter()
            .map(|c| c.text)
            .collect::<Vec<String>>()
            .join("\n");

        StorableMessage {
            role: msg.role,
            content,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<aws_sdk_bedrockruntime::types::ContentBlock> for &AnthropicContent {
    fn into(self) -> aws_sdk_bedrockruntime::types::ContentBlock {
        aws_sdk_bedrockruntime::types::ContentBlock::Text(self.text.clone())
    }
}

impl TryInto<aws_sdk_bedrockruntime::types::Message> for &AnthropicMessage {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<aws_sdk_bedrockruntime::types::Message, Self::Error> {
        Ok(aws_sdk_bedrockruntime::types::Message::builder()
            .role(match self.role.as_str() {
                "user" => aws_sdk_bedrockruntime::types::ConversationRole::User,
                _ => aws_sdk_bedrockruntime::types::ConversationRole::Assistant,
            })
            .set_content(Some(
                self.content
                    .iter()
                    .map(|c| c.into())
                    .collect::<Vec<aws_sdk_bedrockruntime::types::ContentBlock>>(),
            ))
            .build()?)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file = File::create("log.txt")?;
    tracing_subscriber::fmt().with_writer(file).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Chat(args) => execute_chat(args).await?,
        Commands::Code(args) => execute_code(args).await?,
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Message {
    /// The prompt by the user.
    pub prompt: String,
    /// A code block to be used as context in the conversation, without referencing a specific file path.
    pub free_context: String,
}

const MODEL_ID: &str = "anthropic.claude-3-haiku-20240307-v1:0";
const CLAUDE_REGION: &str = "us-west-2";

#[async_trait::async_trait]
pub trait AiClient {
    async fn send_message(&self, message: Message)
        -> Result<SendMessageResponse, SendMessageError>;
}

#[derive(Error, Debug)]
pub enum SendMessageError {
    #[error("A malformed code response was received from the model:\n {}", .0)]
    MalformedCode(String),

    #[error("{}", .0)]
    Custom(String),

    #[error("{}", .0)]
    ConverseError(#[from] ConverseError),

    #[error("{}", .0)]
    SdkError(#[from] SdkError<ConverseError>),
}

#[derive(Debug)]
pub enum SendMessageResponse {
    Chat(String),
    Code { language: String, code: String },
}

#[cfg(test)]
mod tests {
    use aws_config::BehaviorVersion;
    use aws_sdk_bedrockruntime::{
        operation::converse_stream::ConverseStreamOutput,
        types::{
            ContentBlock, ConversationRole, ConverseStreamOutput as ConverseStreamOutputType,
            Message, SystemContentBlock,
        },
        Client,
    };
    use tracing::{debug, error};

    use super::*;

    const MODEL_ID: &str = "anthropic.claude-3-haiku-20240307-v1:0";
    const CLAUDE_REGION: &str = "us-west-2";

    #[tokio::test]
    async fn test_api() {
        let _ = tracing_subscriber::fmt::try_init();

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(CLAUDE_REGION)
            .load()
            .await;

        let client = Client::new(&sdk_config);

        let res = client
            .converse_stream()
            .model_id(MODEL_ID)
            .system(SystemContentBlock::Text(SYSTEM_PROMPT.into()))
            .messages(
                Message::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(format!(
                        "{}\n\n{}",
                        include_str!("./hello.rs"),
                        "write tests"
                    )))
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        let mut stream = res.stream;
        let mut ai_response = String::new();

        loop {
            let token = stream.recv().await;
            match token {
                Ok(Some(output)) => {
                    debug!("Got token: {:?}", output);
                    let text = get_text(output).unwrap();
                    ai_response.push_str(&text);
                }
                Ok(None) => break,
                Err(e) => {
                    error!("An error occurred: {:?}", e);
                    break;
                }
            }
        }

        println!("{}", ai_response);
    }

    fn get_text(output: ConverseStreamOutputType) -> anyhow::Result<String> {
        match output {
            ConverseStreamOutputType::ContentBlockDelta(ev) => match ev.delta() {
                Some(delta) => Ok(delta.as_text().cloned().unwrap_or("".into())),
                None => Ok("".into()),
            },
            _ => Ok("".into()),
        }
    }
}
