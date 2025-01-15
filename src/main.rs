use aws_config::BehaviorVersion;
// use std::io::{IsTerminal, Read, Stdin};
use aws_sdk_bedrockruntime::{
    error::SdkError,
    operation::converse::ConverseError,
    types::{ContentBlock, ConversationRole, Message as BedrockMessage, SystemContentBlock},
    Client,
};
use std::{fs::File, io::IsTerminal};
use thiserror::Error;
use tokio::{io::AsyncReadExt, time::Instant};

use clap::Parser;
use tracing::{debug, error, info};

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
    // /// The directory to include as additional context.
    // #[arg(short, long)]
    // directory: Option<String>,
}

const SYSTEM_PROMPT: &str = r#"\
You are Q, an expert programmer. You are an assistant who can answer questions about code, and generate code when a request is made by the user.

First, decide if the user is asking a question or making a request. When deciding if the user is asking a question, you should only consider the text passed within the <prompt /> tags, and not anything sent before then. For instance, if the user message includes a lot of code but the prompt is asking a question, then the user is asking a question.

If the user is asking a question, then ignore all of the instructions below and respond to the user in chat form. Your response should be a JSON object according to the following JSON schema:
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["type", "message"],
  "properties": {
    "type": {
      "type": "string",
      "const": "chat"
    },
    "message": {
      "type": "string"
    }
  },
  "additionalProperties": false
}

If the user is making a request, then your response should be a JSON object according to the following schema:
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["type", "message"],
  "properties": {
    "type": {
      "type": "string",
      "const": "code"
    },
    "message": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["language", "code"],
        "properties": {
          "language": {
            "type": "string",
            "description": "Programming language identifier for the code block"
          },
          "code": {
            "type": "string",
            "description": "A valid code block written in the programming language specified by the 'language' field"
          },
          "file_path": {
            "type": "string",
            "description": "Optional file path where the code should be saved"
          }
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
The code should be functional, correct, efficient, and include comments where applicable. The code should adhere to best practices in whatever language the user has provided.

Your code should be an updated version of the code provided by the user. For example, if you are not modifying the user's code but instead adding something on top or below it, the user's code should be included in your response.

An example is provided below:
<example>
<user>
pub fn add(x: f32, y: f32) -> f32 {
    x + y
}

<prompt>Generate tests</prompt>
</user>

<assistant>
{
    "type": "code",
    "message": [
        {
            "language": "rust",
            "code": "pub fn add(x: f32, y: f32) -> f32 {\n    x + y\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_add_positive_numbers() {\n        assert_eq!(add(2.5, 3.7), 6.2);\n    }\n\n    #[test]\n    fn test_add_negative_numbers() {\n        assert_eq!(add(-4.1, -1.3), -5.4);\n    }\n\n    #[test]\n    fn test_add_zero() {\n        assert_eq!(add(0.0, 0.0), 0.0);\n    }\n\n    #[test]\n    fn test_add_small_numbers() {\n        assert_eq!(add(0.00001, 0.00002), 0.00003);\n    }\n}"
        }
    ]
}
</assistant>
</example>
"#;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file = File::create("log.txt")?;
    tracing_subscriber::fmt().with_writer(file).init();

    let cli = Cli::parse();

    let mut stdin = tokio::io::stdin();
    let context = if std::io::stdin().is_terminal() {
        String::new()
    } else {
        let mut buf = Vec::with_capacity(256);
        stdin.read_to_end(&mut buf).await?;
        String::from_utf8_lossy(&buf).to_string()
    };

    let prompt = cli.prompt.join(" ");

    info!("Prompt: {}", prompt);
    info!("Context: {:?}", context);

    let client = new_bedrock_client().await;

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

    Ok(())
}

async fn new_bedrock_client() -> impl AiClient {
    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .region(CLAUDE_REGION)
        .load()
        .await;

    let client = Client::new(&sdk_config);

    BedrockClient { client }
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
}

impl BedrockClient {
    pub async fn new() -> Self {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(CLAUDE_REGION)
            .load()
            .await;
        let client = Client::new(&sdk_config);

        Self { client }
    }
}

#[async_trait::async_trait]
impl AiClient for BedrockClient {
    async fn send_message(
        &self,
        message: Message,
    ) -> Result<SendMessageResponse, SendMessageError> {
        let res = self
            .client
            .converse()
            .model_id(MODEL_ID)
            .system(SystemContentBlock::Text(SYSTEM_PROMPT.into()))
            .messages(
                BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(format!(
                        "{}\n\n<prompt>{}</prompt>",
                        message.free_context, message.prompt
                    )))
                    .build()
                    .unwrap(),
            )
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

                #[allow(clippy::manual_strip)]
                // Check if the model responded with code.
                if text.starts_with("```") {
                    debug!("Received code response.");
                    match (text.find("\n"), text[3..].find("```")) {
                        (Some(code_start), Some(code_end)) => {
                            assert!(code_start < code_end);
                            let language = text[3..code_start].into();
                            let code = text[(code_start + 1)..(code_end + 3)].into();
                            Ok(SendMessageResponse::Code { language, code })
                        }
                        _ => Err(SendMessageError::MalformedCode(text.into())),
                    }
                }
                // Otherwise, response should be a user-facing chat message.
                else {
                    debug!("Received chat form response.");
                    println!("{}", text);
                    Ok(SendMessageResponse::Chat(text.into()))
                }
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
