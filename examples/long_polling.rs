use anyhow::Result;
use futures_util::stream::StreamExt;
use ratma_tg_types::{bot::BotBuilder, ext::LongPoller};

#[tokio::main]
async fn main() -> Result<()> {
    let token = std::env::var("TOKEN")?;
    let bot = BotBuilder::new(token)?.build();
    bot.delete_webhook(None).await?;
    let poller = LongPoller::new(&bot, None);
    let mut res = poller.get_updates().await;

    while let Some(Ok(update)) = res.next().await {
        println!("update {:?}", update);
    }
    Ok(())
}
