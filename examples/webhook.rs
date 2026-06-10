use anyhow::{Result, anyhow};
use futures_util::stream::StreamExt;
use ratma_tg_types::{
    bot::BotBuilder,
    ext::{BotUrl, Webhook}
};

#[tokio::main]
async fn main() -> Result<()> {
    let token = std::env::var("TOKEN")?;
    let url = std::env::var("URL")?;

    let url = BotUrl::Host(url);
    let bot = BotBuilder::new(token)?.build();

    let addr = ([0, 0, 0, 0], 8080).into();
    let poller = Webhook::new(&bot, url, false, addr, None);
    let mut res = poller.get_updates().await?;

    while let Some(update) = res.next().await {
        match update {
            Ok(update) => println!("update {:?}", update),
            Err(err) => return Err(anyhow!("updates stream failed: {}", err))
        }
    }
    Ok(())
}
