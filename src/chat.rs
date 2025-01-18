use std::{path::Path, time::Instant};

use clap::Args;

use crate::*;

#[derive(Args, Debug)]
pub struct ChatArgs {
    // This is the id associated with the conversation
    // This is to be joined with the current_repo_dir to form current_repo_dir/.db/resume_chat_ctx
    #[arg(short, long)]
    resume_chat_ctx: String,
    #[arg(short, long)]
    current_repo_dir: String,
    #[arg(short, long)]
    file_ctx: Option<Vec<String>>,
    #[arg(name = "PROMPT")]
    prompt: Vec<String>,
}

pub async fn execute_chat(args: ChatArgs) -> anyhow::Result<()> {
    let prompt = args.prompt.join(" ");

    let current_repo_dir = Path::new(&args.current_repo_dir);
    if !current_repo_dir.is_dir() {
        anyhow::bail!("current repo directory given is invalid");
    }
    let db_path = Path::join(current_repo_dir, ".db");
    if !db_path.is_dir() {
        tokio::fs::create_dir(Path::join(current_repo_dir, ".db")).await?;
    }

    let mut stdin = tokio::io::stdin();
    let mut context = if std::io::stdin().is_terminal() {
        String::new()
    } else {
        let mut buf = Vec::with_capacity(256);
        stdin.read_to_end(&mut buf).await?;
        String::from_utf8_lossy(&buf).to_string()
    };
    let file_ctx = args.file_ctx;
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

    let resume_ctx_path = Path::join(Path::new(".db"), &args.resume_chat_ctx);

    info!("Context: {:?}", context);

    let conversation_history_path = Path::join(current_repo_dir, resume_ctx_path);
    let client = BedrockClient::from_id(&conversation_history_path).await;

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

#[derive(Debug)]
pub struct BedrockClient<'a> {
    client: Client,
    conversation_history: Arc<Mutex<Vec<StorableMessage>>>,
    conversation_id: &'a Path,
}

unsafe impl Send for BedrockClient<'_> {}

impl<'a> BedrockClient<'a> {
    pub async fn from_id(convo_history_path: &'a Path) -> Self {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(CLAUDE_REGION)
            .load()
            .await;
        let client = Client::new(&sdk_config);

        if !convo_history_path.is_file() {
            if let Err(e) = tokio::fs::File::create(convo_history_path).await {
                panic!("Error creating convo history file: {}", e);
            }
        }

        let bedrock_client = Self {
            client,
            conversation_history: Arc::new(Mutex::new(Vec::new())),
            conversation_id: convo_history_path,
        };

        if let Ok(ctx_buf) = tokio::fs::read_to_string(convo_history_path).await {
            if let Ok(mut previous_messages) =
                serde_json::from_str::<Vec<StorableMessage>>(&ctx_buf)
            {
                let mut history = bedrock_client.conversation_history.lock().await;
                history.append(&mut previous_messages);
            }
        }

        bedrock_client
    }

    pub async fn store_conversation(&self) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&*self.conversation_history.lock().await)?;
        tokio::fs::write(self.conversation_id, json).await?;
        Ok(())
    }

    // unused for now
    #[allow(dead_code)]
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
impl AiClient for BedrockClient<'_> {
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
