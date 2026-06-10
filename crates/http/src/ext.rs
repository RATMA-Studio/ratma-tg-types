use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll}
};

use anyhow::{Result, anyhow};
use async_stream::stream;
use futures_core::Stream;
use http_body_util::{BodyExt, Limited};
use hyper::{
    Request, Response, StatusCode,
    body::{Buf, Incoming},
    service::service_fn
};
use hyper_util::rt::TokioIo;
use rand::{TryRng, rngs::SysRng};
use tokio::{
    net::TcpListener,
    sync::{mpsc, mpsc::error::SendError},
    task::{AbortHandle, JoinHandle}
};
use uuid::Uuid;

use crate::{
    bot::{ApiError, Bot},
    gen_types::{Update, UpdateExt}
};

/// Helper for fetching updates via long polling.
pub struct LongPoller {
    bot:             Bot,
    offset:          i64,
    allowed_updates: Option<Vec<String>>
}

impl LongPoller {
    pub fn new(bot: &Bot, allowed_updates: Option<Vec<String>>) -> Self {
        Self {
            bot: bot.clone(),
            offset: 0,
            allowed_updates
        }
    }

    /// Return an async stream of updates, terminating with error
    pub async fn get_updates(
        mut self
    ) -> Pin<Box<impl Stream<Item = Result<UpdateExt, ApiError>>>> {
        let s = stream! {
            loop {
                match self.bot.get_updates(Some(self.offset), None, None, self.allowed_updates.as_ref()).await {
                    Ok(update) => {
                        let mut max = 0;
                        for update in update {
                            let id = update.get_update_id();
                            if id > max {
                                max = id;
                            }
                            yield Ok(update.into());
                        }

                        self.offset = max + 1;
                    }
                    Err(err) => log::warn!("failed to fetch update {}", err)
                }
            }
        };

        Box::pin(s)
    }
}

/// Stream of updates returned by [`Webhook::get_updates`].
///
/// The webhook listener runs as a background tokio task whose failure is
/// never silent: if the task panics or stops, the stream removes the webhook
/// and terminates with an `Err` item describing the task outcome. Dropping
/// the stream aborts the listener task; [`WebhookStream::abort`] does the
/// same explicitly.
pub struct WebhookStream {
    stream: Pin<Box<dyn Stream<Item = Result<UpdateExt, ApiError>> + Send>>,
    abort:  AbortHandle
}

impl WebhookStream {
    /// Stops the background listener task. The stream then removes the
    /// webhook and terminates with an `Err` item reporting the cancellation.
    pub fn abort(&self) {
        self.abort.abort();
    }
}

impl Stream for WebhookStream {
    type Item = Result<UpdateExt, ApiError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().stream.as_mut().poll_next(cx)
    }
}

impl Drop for WebhookStream {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

/// Endpoint for webhooks, could be either a raw ip address or a hostname
pub enum BotUrl {
    Address(String, IpAddr),
    Host(String)
}

/// Helper for fetching updates via webhook. This currently requires a reverse
/// proxy as tls is not supported.
pub struct Webhook {
    bot:                  Bot,
    url:                  BotUrl,
    drop_pending_updates: bool,
    addr:                 SocketAddr,
    cookie:               Uuid,
    allowed_updates:      Option<Vec<String>>
}

impl Webhook {
    pub fn new(
        bot: &Bot,
        url: BotUrl,
        drop_pending_updates: bool,
        addr: SocketAddr,
        allowed_updates: Option<Vec<String>>
    ) -> Self {
        let mut bytes: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        SysRng.try_fill_bytes(&mut bytes).unwrap();
        let cookie = Uuid::from_slice(bytes.as_slice()).expect("invalid uuid");
        Self {
            bot: bot.clone(),
            url,
            drop_pending_updates,
            addr,
            cookie,
            allowed_updates
        }
    }

    async fn setup(&self) -> Result<bool, ApiError> {
        match self.url {
            BotUrl::Address(ref addr, ip) => {
                self.bot
                    .set_webhook(
                        addr,
                        None,
                        Some(&ip.to_string()),
                        None,
                        self.allowed_updates.as_ref(),
                        Some(self.drop_pending_updates),
                        Some(self.cookie.to_string().as_str())
                    )
                    .await
            }
            BotUrl::Host(ref host) => {
                self.bot
                    .set_webhook(
                        host,
                        None,
                        None,
                        None,
                        self.allowed_updates.as_ref(),
                        Some(self.drop_pending_updates),
                        Some(self.cookie.to_string().as_str())
                    )
                    .await
            }
        }
    }

    async fn teardown(&self) -> Result<bool, ApiError> {
        self.bot
            .delete_webhook(Some(self.drop_pending_updates))
            .await
    }

    /// Spawns the task listening for incoming webhook connections and
    /// forwarding decoded updates into `tx`.
    fn spawn_listener(
        &self,
        listener: TcpListener,
        tx: mpsc::Sender<UpdateExt>
    ) -> JoinHandle<()> {
        let cookie = self.cookie;
        let svc = service_fn(move |body: Request<Incoming>| {
            let tx = tx.clone();
            async move {
                if let Some(token) = body.headers().get("X-Telegram-Bot-Api-Secret-Token")
                    && token.to_str().unwrap_or("") == cookie.to_string().as_str()
                {
                    let body = Limited::new(body, 1024 * 1024 * 10);
                    let body = body.collect().await.map_err(|e| anyhow!(e))?.aggregate();
                    if let Ok(update) = serde_json::from_reader::<_, Update>(body.reader()) {
                        tx.send(update.into())
                            .await
                            .map_err(|e: SendError<UpdateExt>| anyhow!(e))?;
                    }
                }
                Ok::<_, ApiError>(
                    Response::builder()
                        .status(StatusCode::OK)
                        .body("".to_owned())
                        .map_err(|e| anyhow!(e))?
                )
            }
        });
        tokio::spawn(async move {
            loop {
                let svc = svc.clone();
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let io = TokioIo::new(stream);

                        tokio::task::spawn(async move {
                            if let Err(err) = hyper::server::conn::http1::Builder::new()
                                .serve_connection(io, svc)
                                .await
                            {
                                log::warn!("connection error {}", err);
                            }
                        });
                    }
                    Err(err) => log::warn!("failed to accept webhook connection {}", err)
                }
            }
        })
    }

    /// Wraps the update channel and the listener task into the public stream.
    ///
    /// The stream yields updates until the listener task completes (which a
    /// task running an infinite accept loop only does by panicking or being
    /// aborted), then removes the webhook and terminates with an `Err` item
    /// describing the task outcome.
    fn into_stream(
        self,
        mut rx: mpsc::Receiver<UpdateExt>,
        mut handle: JoinHandle<()>
    ) -> WebhookStream {
        let abort = handle.abort_handle();
        let s = stream! {
            let join_result = loop {
                tokio::select! {
                    biased;
                    result = &mut handle => break result,
                    update = rx.recv() => {
                        if let Some(update) = update {
                            yield Ok(update);
                        }
                    }
                }
            };

            if let Err(err) = self.teardown().await {
                log::warn!("failed to remove webhook during shutdown {}", err);
            }

            match join_result {
                Ok(()) => yield Err(anyhow!("webhook listener task stopped unexpectedly").into()),
                Err(err) => yield Err(anyhow!("webhook listener task failed: {}", err).into())
            }
        };

        WebhookStream {
            stream: Box::pin(s),
            abort
        }
    }

    /// Enable the webhook and return an async stream of updates.
    ///
    /// The returned [`WebhookStream`] owns the background listener task: a
    /// panic or unexpected exit of that task surfaces as the terminal `Err`
    /// item of the stream instead of silently stopping updates, and dropping
    /// the stream aborts the task.
    pub async fn get_updates(self) -> Result<WebhookStream, ApiError> {
        let (tx, rx) = mpsc::channel(128);

        let listener = TcpListener::bind(self.addr).await.map_err(|e| anyhow!(e))?;
        let handle = self.spawn_listener(listener, tx);

        if let Err(err) = self.setup().await {
            handle.abort();
            self.teardown().await?;
            return Err(err);
        }

        Ok(self.into_stream(rx, handle))
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::*;
    use crate::bot::BotBuilder;

    // Bot pointing at a closed local port: any API call fails fast without
    // touching the network beyond loopback.
    fn webhook() -> Webhook {
        let bot = BotBuilder::new("token")
            .expect("client builder failed")
            .api("https://127.0.0.1:1")
            .build();
        Webhook::new(
            &bot,
            BotUrl::Host("https://example.com".to_owned()),
            false,
            ([127, 0, 0, 1], 0).into(),
            None
        )
    }

    #[tokio::test]
    async fn listener_panic_terminates_stream_with_error() {
        let (_tx, rx) = mpsc::channel(1);
        let handle = tokio::spawn(async { panic!("listener crashed") });
        let mut stream = webhook().into_stream(rx, handle);

        let item = stream
            .next()
            .await
            .expect("stream must yield a terminal item");
        let err = item.expect_err("listener panic must surface as an error");
        assert!(
            err.to_string().contains("webhook listener task failed"),
            "unexpected error: {err}"
        );
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn abort_terminates_stream_with_error() {
        let (_tx, rx) = mpsc::channel(1);
        let handle = tokio::spawn(std::future::pending::<()>());
        let mut stream = webhook().into_stream(rx, handle);
        stream.abort();

        let item = stream
            .next()
            .await
            .expect("stream must yield a terminal item");
        assert!(item.is_err(), "abort must surface as an error");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn updates_are_forwarded_before_termination() {
        let (tx, rx) = mpsc::channel(1);
        let handle = tokio::spawn(std::future::pending::<()>());
        let mut stream = webhook().into_stream(rx, handle);

        tx.send(UpdateExt::Invalid).await.expect("send failed");
        let item = stream.next().await.expect("stream must yield the update");
        assert!(matches!(item, Ok(UpdateExt::Invalid)));
    }
}
