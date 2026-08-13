// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use crate::CodexError;

pub(crate) struct Connection<R, W> {
    reader: R,
    writer: W,
    next_request_id: u64,
    pending_server_messages: VecDeque<ServerMessage>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ServerMessage {
    Notification {
        method: String,
        params: Value,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
}

impl<R, W> Connection<R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub(crate) fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_request_id: 1,
            pending_server_messages: VecDeque::new(),
        }
    }

    pub(crate) async fn request<P, T>(
        &mut self,
        method: &'static str,
        params: &P,
    ) -> Result<T, CodexError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let request = Request {
            id: request_id,
            method,
            params,
        };
        self.write_message(&request).await?;

        loop {
            let message = self.read_message(method).await?;
            if message.get("method").is_some() {
                self.pending_server_messages
                    .push_back(server_message(message, method)?);
                continue;
            }
            let Some(response_id) = message.get("id") else {
                return Err(CodexError::UnexpectedMessage {
                    method,
                    description: "message has neither a method nor a response identifier"
                        .to_owned(),
                });
            };
            if response_id.as_u64() != Some(request_id) {
                return Err(CodexError::UnexpectedMessage {
                    method,
                    description: "response with an unknown request identifier".to_owned(),
                });
            }

            if let Some(result) = message.get("result") {
                return serde_json::from_value(result.clone())
                    .map_err(|source| CodexError::InvalidResponse { method, source });
            }
            if let Some(error) = message.get("error") {
                let error: RpcError = serde_json::from_value(error.clone())
                    .map_err(|source| CodexError::InvalidResponse { method, source })?;
                return Err(CodexError::Server {
                    method,
                    code: error.code,
                    message: error.message,
                });
            }
            return Err(CodexError::UnexpectedMessage {
                method,
                description: "matching response has neither result nor error".to_owned(),
            });
        }
    }

    pub(crate) async fn next_server_message(
        &mut self,
        operation: &'static str,
    ) -> Result<ServerMessage, CodexError> {
        if let Some(message) = self.pending_server_messages.pop_front() {
            return Ok(message);
        }
        let message = self.read_message(operation).await?;
        server_message(message, operation)
    }

    pub(crate) async fn respond_result(
        &mut self,
        id: Value,
        result: Value,
    ) -> Result<(), CodexError> {
        self.write_message(&Response { id, result }).await
    }

    pub(crate) async fn respond_error(
        &mut self,
        id: Value,
        code: i64,
        message: String,
    ) -> Result<(), CodexError> {
        self.write_message(&ErrorResponse {
            id,
            error: ResponseError { code, message },
        })
        .await
    }

    pub(crate) async fn initialized(&mut self) -> Result<(), CodexError> {
        self.write_message(&InitializedNotification {
            method: "initialized",
        })
        .await
    }

    async fn read_message(&mut self, operation: &'static str) -> Result<Value, CodexError> {
        let mut line = Vec::new();
        let byte_count = self
            .reader
            .read_until(b'\n', &mut line)
            .await
            .map_err(|source| CodexError::io("read from", source))?;
        if byte_count == 0 {
            return Err(CodexError::UnexpectedEof(operation));
        }
        serde_json::from_slice(&line).map_err(CodexError::InvalidJson)
    }

    async fn write_message(&mut self, message: &impl Serialize) -> Result<(), CodexError> {
        let encoded = serde_json::to_vec(message).map_err(CodexError::Encode)?;
        self.writer
            .write_all(&encoded)
            .await
            .map_err(|source| CodexError::io("write to", source))?;
        self.writer
            .write_all(b"\n")
            .await
            .map_err(|source| CodexError::io("write to", source))?;
        self.writer
            .flush()
            .await
            .map_err(|source| CodexError::io("flush", source))
    }

    #[cfg(test)]
    pub(crate) fn writer(&self) -> &W {
        &self.writer
    }
}

fn server_message(message: Value, operation: &'static str) -> Result<ServerMessage, CodexError> {
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| CodexError::UnexpectedMessage {
            method: operation,
            description: "expected a server notification or request".to_owned(),
        })?
        .to_owned();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Ok(match message.get("id") {
        Some(id) => ServerMessage::Request {
            id: id.clone(),
            method,
            params,
        },
        None => ServerMessage::Notification { method, params },
    })
}

#[derive(Serialize)]
struct Request<'a, P> {
    id: u64,
    method: &'static str,
    params: &'a P,
}

#[derive(Serialize)]
struct InitializedNotification {
    method: &'static str,
}

#[derive(Serialize)]
struct Response {
    id: Value,
    result: Value,
}

#[derive(Serialize)]
struct ErrorResponse {
    id: Value,
    error: ResponseError,
}

#[derive(Serialize)]
struct ResponseError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde::{Deserialize, Serialize};
    use tokio::io::BufReader;

    use super::*;

    #[derive(Serialize)]
    struct TestParams {
        value: u32,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct TestResponse {
        accepted: bool,
    }

    #[tokio::test]
    async fn matches_a_response_and_preserves_an_interleaved_notification() {
        let input = concat!(
            "{\"method\":\"thread/started\",\"params\":{}}\n",
            "{\"id\":1,\"result\":{\"accepted\":true}}\n"
        );
        let reader = BufReader::new(Cursor::new(input.as_bytes()));
        let mut connection = Connection::new(reader, Vec::new());

        let response: TestResponse = connection
            .request("test/read", &TestParams { value: 7 })
            .await
            .expect("the matching response should decode");

        assert_eq!(response, TestResponse { accepted: true });
        assert_eq!(
            connection.next_server_message("test/stream").await.unwrap(),
            ServerMessage::Notification {
                method: "thread/started".to_owned(),
                params: serde_json::json!({}),
            }
        );
        assert_eq!(
            String::from_utf8(connection.writer).expect("the request should be UTF-8"),
            "{\"id\":1,\"method\":\"test/read\",\"params\":{\"value\":7}}\n"
        );
    }

    #[tokio::test]
    async fn preserves_and_answers_an_interleaved_server_request() {
        let input = concat!(
            "{\"id\":\"approval-1\",\"method\":\"item/fileChange/requestApproval\",\"params\":{\"threadId\":\"thread-1\"}}\n",
            "{\"id\":1,\"result\":{\"accepted\":true}}\n"
        );
        let reader = BufReader::new(Cursor::new(input.as_bytes()));
        let mut connection = Connection::new(reader, Vec::new());

        let _: TestResponse = connection
            .request("test/read", &TestParams { value: 7 })
            .await
            .expect("the matching response should decode");
        assert_eq!(
            connection.next_server_message("test/stream").await.unwrap(),
            ServerMessage::Request {
                id: serde_json::json!("approval-1"),
                method: "item/fileChange/requestApproval".to_owned(),
                params: serde_json::json!({ "threadId": "thread-1" }),
            }
        );

        connection
            .respond_result(
                serde_json::json!("approval-1"),
                serde_json::json!({ "decision": "decline" }),
            )
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8(connection.writer).unwrap(),
            concat!(
                "{\"id\":1,\"method\":\"test/read\",\"params\":{\"value\":7}}\n",
                "{\"id\":\"approval-1\",\"result\":{\"decision\":\"decline\"}}\n"
            )
        );
    }
}
