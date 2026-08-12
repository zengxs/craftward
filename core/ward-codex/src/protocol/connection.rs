// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{BufRead, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CodexError;

pub(crate) struct Connection<R, W> {
    reader: R,
    writer: W,
    next_request_id: u64,
}

impl<R, W> Connection<R, W>
where
    R: BufRead,
    W: Write,
{
    pub(crate) fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_request_id: 1,
        }
    }

    pub(crate) fn request<P, T>(
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
        serde_json::to_writer(&mut self.writer, &request).map_err(CodexError::Encode)?;
        self.writer
            .write_all(b"\n")
            .map_err(|source| CodexError::io("write to", source))?;
        self.writer
            .flush()
            .map_err(|source| CodexError::io("flush", source))?;

        let mut line = String::new();
        loop {
            line.clear();
            let byte_count = self
                .reader
                .read_line(&mut line)
                .map_err(|source| CodexError::io("read from", source))?;
            if byte_count == 0 {
                return Err(CodexError::UnexpectedEof(method));
            }

            let message: Value = serde_json::from_str(&line).map_err(CodexError::InvalidJson)?;
            let Some(response_id) = message.get("id") else {
                // Notifications may be interleaved with responses. The read-only
                // interface has no notification consumer yet, so they are ignored.
                continue;
            };
            if response_id.as_u64() != Some(request_id) {
                let description = match message.get("method").and_then(Value::as_str) {
                    Some(server_method) => format!("server request {server_method}"),
                    None => "response with an unknown request identifier".to_owned(),
                };
                return Err(CodexError::UnexpectedMessage {
                    method,
                    description,
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

    pub(crate) fn initialized(&mut self) -> Result<(), CodexError> {
        serde_json::to_writer(
            &mut self.writer,
            &InitializedNotification {
                method: "initialized",
            },
        )
        .map_err(CodexError::Encode)?;
        self.writer
            .write_all(b"\n")
            .map_err(|source| CodexError::io("write to", source))?;
        self.writer
            .flush()
            .map_err(|source| CodexError::io("flush", source))
    }
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

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize)]
    struct TestParams {
        value: u32,
    }

    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct TestResponse {
        accepted: bool,
    }

    #[test]
    fn matches_a_response_after_an_interleaved_notification() {
        let input = concat!(
            "{\"method\":\"thread/started\",\"params\":{}}\n",
            "{\"id\":1,\"result\":{\"accepted\":true}}\n"
        );
        let reader = BufReader::new(Cursor::new(input.as_bytes()));
        let mut connection = Connection::new(reader, Vec::new());

        let response: TestResponse = connection
            .request("test/read", &TestParams { value: 7 })
            .expect("the matching response should decode");

        assert_eq!(response, TestResponse { accepted: true });
        assert_eq!(
            String::from_utf8(connection.writer).expect("the request should be UTF-8"),
            "{\"id\":1,\"method\":\"test/read\",\"params\":{\"value\":7}}\n"
        );
    }
}
