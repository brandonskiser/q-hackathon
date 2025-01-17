use aws_config::BehaviorVersion;
// use std::io::{IsTerminal, Read, Stdin};
use aws_sdk_bedrockruntime::{
    error::SdkError,
    operation::converse::ConverseError,
    types::{ContentBlock, ConversationRole, Message as BedrockMessage, SystemContentBlock},
    Client,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{borrow::BorrowMut, fs::File, io::IsTerminal, path::Path, sync::Arc};
use thiserror::Error;
use tokio::{io::AsyncReadExt, sync::Mutex, time::Instant};

use clap::Parser;
use tracing::{debug, error, info};

mod system_prompts;
use system_prompts::SYSTEM_PROMPT;

/// Simple program?
#[derive(Parser, Debug)]
#[command(
    author = "Amazon Q",
    version,
    about = "An AI-powdered CLI for your terminal and editor."
)]
struct Cli {
    #[arg(name = "PROMPT")]
    prompt: Vec<String>,
    #[arg(short, long)]
    file_ctx: Option<Vec<String>>,
    #[arg(short, long)]
    resume_chat_ctx: Option<String>,
    #[arg(short, long)]
    current_repo_dir: String,
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
    let current_repo_dir = Path::new(&cli.current_repo_dir);
    if !current_repo_dir.is_dir() {
        anyhow::bail!("current repo directory given is invalid");
    }
    tokio::fs::create_dir(Path::join(current_repo_dir, ".db")).await?;

    let mut stdin = tokio::io::stdin();
    let mut context = if std::io::stdin().is_terminal() {
        String::new()
    } else {
        let mut buf = Vec::with_capacity(256);
        stdin.read_to_end(&mut buf).await?;
        String::from_utf8_lossy(&buf).to_string()
    };
    let file_ctx = cli.file_ctx;
    if let Some(file_ctx) = file_ctx {
        for ctx in file_ctx {
            let path = Path::new(&ctx);
            if path.is_file() {
                let buf = tokio::fs::read_to_string(path).await;
                if let Ok(buf) = buf {
                    context.push_str(&buf);
                } else {
                    info!(
                        "{} is not a file, skipping.",
                        path.to_str().unwrap_or("bad path")
                    );
                }
            }
        }
    }

    let prompt = cli.prompt.join(" ");
    let resume_ctx = cli.resume_chat_ctx.as_ref().map(Path::new);

    info!("Context: {:?}", context);

    let client = BedrockClient::from_id(resume_ctx, current_repo_dir).await;

    let response = {
        let start = Instant::now();
        let res = client
            .send_message(Message {
                prompt,
                free_context: context,
            })
            .await?;
        let end = Instant::now();
        debug!("Response took {} ms", (end - start).as_millis());
        res
    };

    match response {
        SendMessageResponse::Chat(chat) => println!("{}", chat),
        SendMessageResponse::Code { language, code } => {
            debug!("Model responded with language: {}", language);
            println!("{}", code)
        }
    }

    client.store_conversation().await?;

    Ok(())
}

async fn retrieve_existing_convo_context(id: &Path) -> anyhow::Result<String> {
    if !id.is_file() {
        anyhow::bail!("id given is not a file");
    }
    Ok(tokio::fs::read_to_string(id).await?)
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

#[derive(Debug)]
pub struct BedrockClient {
    client: Client,
    conversation_history: Arc<Mutex<Vec<StorableMessage>>>,
    conversation_id: Option<String>,
}

impl BedrockClient {
    pub async fn new() -> Self {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(CLAUDE_REGION)
            .load()
            .await;
        let client = Client::new(&sdk_config);

        Self {
            client,
            conversation_history: Arc::new(Mutex::new(Vec::new())),
            conversation_id: None,
        }
    }

    pub async fn from_id(convo_id: Option<&Path>, repo_dir: &Path) -> Self {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(CLAUDE_REGION)
            .load()
            .await;
        let client = Client::new(&sdk_config);

        let bedrock_client = Self {
            client,
            conversation_history: Arc::new(Mutex::new(Vec::new())),
            conversation_id: Some(".db/some_unique_id".to_string()),
        };

        if let Some(convo_id) = convo_id {
            if convo_id.is_file() {
                if let Ok(ctx_buf) = tokio::fs::read_to_string(convo_id).await {
                    if let Ok(mut previous_messages) =
                        serde_json::from_str::<Vec<StorableMessage>>(&ctx_buf)
                    {
                        let mut history = bedrock_client.conversation_history.lock().await;
                        history.append(&mut previous_messages);
                    }
                }
            } else if let Err(e) = tokio::fs::File::create(convo_id).await {
                panic!("Cannot create context file: {}", e);
            }
        } else if let Err(e) =
            tokio::fs::File::create(Path::join(repo_dir, ".db/some_unique_id")).await
        {
            panic!("Cannot create context file: {}", e);
        }

        bedrock_client
    }

    pub async fn store_conversation(&self) -> anyhow::Result<()> {
        println!("running store conversation");
        if let Some(ctx_file_path) = &self.conversation_id {
            let json = serde_json::to_string_pretty(&*self.conversation_history.lock().await)?;
            println!("*****json to be written is: {}", json);
            tokio::fs::write(ctx_file_path, json).await?;
            return Ok(());
        }
        anyhow::bail!("Error storing conversation. Ctx file path not provided");
    }

    pub async fn load_conversation<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let json = tokio::fs::read_to_string(path).await?;
        let mut history = self.conversation_history.lock().await;
        let new_history = serde_json::from_str::<Vec<StorableMessage>>(&json)?;
        std::mem::take(&mut *history);
        *history = new_history;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AiClient for BedrockClient {
    async fn send_message(
        &self,
        message: Message,
    ) -> Result<SendMessageResponse, SendMessageError> {
        let mut history = self.conversation_history.lock().await;
        let mut prompt = String::new();
        if !message.free_context.is_empty() {
            prompt.push_str(&format!("{}\n", message.free_context));
        }
        prompt.push_str(&message.prompt);
        history.push(StorableMessage {
            role: "user".to_string(),
            content: prompt,
        });
        let messages = {
            history
                .iter()
                .filter_map(|m| {
                    let anth_msg: AnthropicMessage = m.into();
                    let br_msg: Result<aws_sdk_bedrockruntime::types::Message, _> =
                        (&anth_msg).try_into();
                    br_msg.ok()
                })
                .collect::<Vec<aws_sdk_bedrockruntime::types::Message>>()
        };
        let res = self
            .client
            .converse()
            .model_id(MODEL_ID)
            .system(SystemContentBlock::Text(SYSTEM_PROMPT.into()))
            .set_messages(Some(messages))
            .send()
            .await;

        debug!("Received response: {:?}", res);

        match res {
            Ok(res) => {
                let text = res
                    .output()
                    .ok_or(SendMessageError::Custom("No output was received".into()))?
                    .as_message()
                    .map_err(|_| {
                        SendMessageError::Custom("Unknown response received from the model".into())
                    })?
                    .content()
                    .first()
                    .ok_or(SendMessageError::Custom(
                        "No content exists in the model response".into(),
                    ))?
                    .as_text()
                    .map_err(|_| SendMessageError::Custom("Model response was not text".into()))?;

                let cleaned_text = text.replace("\\n", "\\\\n");
                history.push(StorableMessage {
                    role: "assistant".to_string(),
                    content: cleaned_text.clone(),
                });
                Ok(SendMessageResponse::Chat(cleaned_text))
            }
            Err(err) => match err {
                aws_smithy_runtime_api::client::result::SdkError::ServiceError(service_error) => {
                    Err(service_error.into_err().into())
                }
                err => Err(err.into()),
            },
        }
    }
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
