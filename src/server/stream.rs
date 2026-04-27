//! SSE 流桥接 —— 泛型 Stream 转 axum Body
//!
//! 支持 OpenAI 与 Anthropic 两种流式响应。

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::{
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use pin_project_lite::pin_project;
use tokio::time::Sleep;

// ---------------------------------------------------------------------------
// KeepaliveStream —— 上游空闲时定期发送 SSE 保活注释
// ---------------------------------------------------------------------------

pin_project! {
    /// SSE 保活流：上游空闲超过 `interval` 时自动注入 `: keepalive\n\n`
    ///
    /// 工具解析（`CollectingXml`）和修复等待（`Repairing`）期间，上游可能长时
    /// 间不产生数据。此 wrapper 在空闲超时后发送 SSE 标准保活注释，防止客户端
    /// 因收不到任何数据而超时断开连接。
    struct KeepaliveStream<S> {
        #[pin]
        inner: S,
        #[pin]
        deadline: Sleep,
        interval: Duration,
    }
}

impl<S> KeepaliveStream<S> {
    fn new(inner: S, interval: Duration) -> Self {
        Self {
            inner,
            deadline: tokio::time::sleep_until(tokio::time::Instant::now() + interval),
            interval,
        }
    }
}

impl<S, E> Stream for KeepaliveStream<S>
where
    S: Stream<Item = Result<Bytes, E>>,
{
    type Item = Result<Bytes, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // 优先检查上游是否有数据
        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // 有数据产出 → 重置保活计时器
                this.deadline
                    .as_mut()
                    .reset(tokio::time::Instant::now() + *this.interval);
                return Poll::Ready(Some(Ok(bytes)));
            }
            Poll::Ready(item) => return Poll::Ready(item),
            Poll::Pending => {}
        }

        // 上游无数据 → 检查保活计时器是否到期
        if this.deadline.as_mut().poll(cx).is_ready() {
            this.deadline
                .as_mut()
                .reset(tokio::time::Instant::now() + *this.interval);
            Poll::Ready(Some(Ok(Bytes::from(": keepalive\n\n"))))
        } else {
            Poll::Pending
        }
    }
}

// ---------------------------------------------------------------------------
// SseBody
// ---------------------------------------------------------------------------

/// SSE 响应体包装器（泛型）
pub struct SseBody<S> {
    inner: S,
    extra_headers: Vec<(String, String)>,
}

impl<S, E> SseBody<S>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::fmt::Display + Send + Sync + 'static,
{
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream,
            extra_headers: Vec::new(),
        }
    }

    /// 添加自定义响应头
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.extra_headers
            .push((name.to_string(), value.to_string()));
        self
    }
}

impl<S, E> IntoResponse for SseBody<S>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::fmt::Display + Send + Sync + 'static,
{
    fn into_response(self) -> Response {
        const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(1700);

        // 用 KeepaliveStream 包装内层流，空闲超时自动发送保活
        let keepalive = KeepaliveStream::new(self.inner, KEEPALIVE_INTERVAL);

        let body = Body::from_stream(keepalive.map(|result| {
            result.map_err(|e| {
                log::error!(target: "http::response", "SSE stream error: {}", e);
                std::io::Error::other(e.to_string())
            })
        }));

        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive");

        for (name, value) in self.extra_headers {
            builder = builder.header(&name, &value);
        }

        builder.body(body).unwrap().into_response()
    }
}
