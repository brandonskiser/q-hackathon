use std::path::Path;

use clap::Args;

use crate::*;

#[derive(Args, Debug)]
pub struct CodeArgs {
    #[arg(short, long)]
    file_ctx: Option<Vec<String>>,
    #[arg(name = "PROMPT")]
    prompt: Vec<String>,
}

pub async fn execute_code(args: CodeArgs) -> anyhow::Result<()> {
    // let prompt = args.prompt.join(" ");
    //
    // let mut stdin = tokio::io::stdin();
    // let mut context = if std::io::stdin().is_terminal() {
    //     String::new()
    // } else {
    //     let mut buf = Vec::with_capacity(256);
    //     stdin.read_to_end(&mut buf).await?;
    //     String::from_utf8_lossy(&buf).to_string()
    // };
    //
    // let file_ctx = args.file_ctx;
    // if let Some(file_ctx) = file_ctx {
    //     for ctx in file_ctx {
    //         let path = Path::new(&ctx);
    //         if path.is_file() {
    //             let buf = tokio::fs::read_to_string(path).await;
    //             if let Ok(buf) = buf {
    //                 context.push_str(&buf);
    //             } else {
    //                 info!(
    //                     "{} is not a file, skipping.",
    //                     path.to_str().unwrap_or("bad path")
    //                 );
    //             }
    //         }
    //     }
    // }
    //
    // let client = BedrockClient::new().await;
    // let response = {
    //     let start = Instant::now();
    //     let res = client
    //         .send_message(Message {
    //             prompt,
    //             free_context: context,
    //         })
    //         .await?;
    //     let end = Instant::now();
    //     debug!("Response took {} ms", (end - start).as_millis());
    //     res
    // };
    // match response {
    //     SendMessageResponse::Chat(chat) => println!("{}", chat),
    //     SendMessageResponse::Code { language, code } => {
    //         debug!("Model responded with language: {}", language);
    //         println!("{}", code)
    //     }
    // }
    Ok(())
}

struct BedrockClient {}
