use std::{iter::Peekable, path::Path, str::Lines};

use anyhow::bail;
use aws_sdk_bedrockruntime::{
    error::SdkError,
    operation::converse::ConverseError,
    types::{ContentBlock, ConversationRole, Message as BedrockMessage, SystemContentBlock},
    Client,
};
use clap::Args;
use system_prompts::CODE_PROMPT;

use crate::*;

#[derive(Args, Debug)]
pub struct CodeArgs {
    #[arg(short, long)]
    file_ctx: Option<Vec<String>>,
    #[arg(name = "PROMPT")]
    prompt: Vec<String>,
}

pub async fn execute_code(args: CodeArgs) -> anyhow::Result<()> {
    let prompt = args.prompt.join(" ");
    debug!(prompt, "parsed prompt");

    let mut stdin = tokio::io::stdin();
    let mut free_context = if std::io::stdin().is_terminal() {
        String::new()
    } else {
        let mut buf = Vec::with_capacity(256);
        stdin.read_to_end(&mut buf).await?;
        String::from_utf8_lossy(&buf).to_string()
    };

    debug!(free_context, "read free context from stdin");

    let file_ctx = args.file_ctx;
    if let Some(file_ctx) = file_ctx {
        for ctx in file_ctx {
            let path = Path::new(&ctx);
            if path.is_file() {
                let buf = tokio::fs::read_to_string(path).await;
                if let Ok(buf) = buf {
                    free_context.push_str(&buf);
                } else {
                    info!(
                        "{} is not a file, skipping.",
                        path.to_str().unwrap_or("bad path")
                    );
                }
            }
        }
    }

    let client = BedrockClient::new().await;
    let response = {
        let start = Instant::now();
        let res = client
            .send_message(format!("{}\n\n<prompt>{}</prompt>", free_context, prompt))
            .await?;
        let end = Instant::now();
        debug!("Response took {} ms", (end - start).as_millis());
        res
    };

    let code_objects = ResponseParser::new(&response.message).parse()?;
    println!(
        "{}",
        serde_json::to_string(&CodeResponse {
            kind: Kind::Code,
            message: code_objects
        })?
    );

    Ok(())
}

struct ResponseParser<'a> {
    iter: Peekable<Lines<'a>>,
    result: Vec<CodeObject>,
}

impl<'a> ResponseParser<'a> {
    fn new(response: &'a str) -> Self {
        Self {
            iter: response.lines().peekable(),
            result: Vec::new(),
        }
    }

    fn has_next(&mut self) -> bool {
        self.iter.peek().is_some()
    }

    fn next(&mut self) {
        self.iter.next();
    }

    fn parse(mut self) -> anyhow::Result<Vec<CodeObject>> {
        while self.has_next() {
            if self.iter.peek().unwrap().starts_with("```") {
                let block = self.parse_block()?;
                self.result.push(block);
            } else {
                self.next();
            }
        }

        Ok(self.result)
    }

    fn parse_block(&mut self) -> anyhow::Result<CodeObject> {
        let markdown_start = self.iter.next().unwrap();
        let language = markdown_start[3..].to_string();
        let mut code = String::new();
        while let Some(line) = self.iter.peek() {
            if line.starts_with("```") {
                self.iter.next();
                break;
            }
            code.push_str(line);
            code.push_str("\\n");
            self.next();
        }
        Ok(CodeObject {
            language,
            code,
            file_path: None,
        })
    }
}

#[derive(Debug)]
struct BedrockClient {
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

    pub async fn send_message(
        &self,
        message: String,
    ) -> Result<SendMessageResponse, SendMessageError> {
        debug!("Sending message: {:?}", message);

        let res = self
            .client
            .converse()
            .model_id(MODEL_ID)
            .system(SystemContentBlock::Text(CODE_PROMPT.into()))
            .messages(
                BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(message))
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

                Ok(SendMessageResponse {
                    message: text.to_string(),
                })
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

#[derive(Debug)]
struct SendMessageResponse {
    message: String,
}

#[derive(Debug, Serialize)]
enum Kind {
    #[serde(rename = "code")]
    Code,
}

#[derive(Debug, Serialize)]
struct CodeResponse {
    #[serde(rename = "type")]
    kind: Kind,
    message: Vec<CodeObject>,
}

#[derive(Debug, Serialize)]
struct CodeObject {
    language: String,
    code: String,
    file_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize() {
        let x = CodeResponse {
            kind: Kind::Code,
            message: vec![CodeObject {
                language: "rust".into(),
                code: "fn main() { println!(\"Hello, world!\"); }".into(),
                file_path: None,
            }],
        };

        println!("{}", serde_json::to_string(&x).unwrap());
    }

    const TEST_RESPONSE: &str = r#"\
```rust
fn main() {
    println!("Hello, World!");
}
```"#;

    #[test]
    fn test_parser() {
        let parser = ResponseParser::new(TEST_RESPONSE);
        println!(
            "{}",
            serde_json::to_string(&parser.parse().unwrap()).unwrap()
        );
    }
}
