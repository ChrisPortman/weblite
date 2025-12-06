use embedded_io_async::{Read, Write};

use crate::ascii::{AsciiInt, CR, LF, SP};
use crate::header::{RequestHeader, ResponseHeader};
use crate::request::Request;
use crate::websocket::{Websocket, sec_websocket_accept_val};
use crate::{HttpWrite, WriteError};

const HTTP_PROTO: &str = "HTTP/1.1";

/// Responder error is returned as the error when responding to clients.  Generally users of the
/// httplite library will not inspect this error, but pass it on from the handler implementations.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ResponderError {
    /// Network error writing data to the client
    NetworkError,
    /// Protocol error parsing supplied data
    ProtocolError(&'static str),
}

impl From<WriteError> for ResponderError {
    fn from(value: WriteError) -> Self {
        match value {
            WriteError::NetworkError => Self::NetworkError,
        }
    }
}

/// HTTP status code returned in a response
#[non_exhaustive]
#[derive(Clone, Copy)]
pub enum StatusCode {
    /// 101 Swithcing Protocols - eg from HTTP to Websocket
    SwitchingProtocols,
    /// 200 Ok
    OK,
    /// 400 Bad Request
    BadRequest,
    /// 404 Not Found
    NotFound,
    /// 500 Server Error
    InternalServerError,
    /// Any other code
    Other(u16),
}

impl HttpWrite for StatusCode {
    #[rustfmt::skip]
    async fn write<T: Write>(self, writer: &mut T) -> Result<(), WriteError> {
        let other: AsciiInt;
        let data = match self {
            Self::SwitchingProtocols => "101 Switching Protocols",
            Self::OK => "200 OK",
            Self::BadRequest => "400 Bad Request",
            Self::NotFound => "404 Not Found",
            Self::InternalServerError => "500 Internal Server Error",
            Self::Other(n) => {
                other = AsciiInt::from(n as u64);
                other.as_str()
            }
        };

        writer.write_all(HTTP_PROTO.as_bytes()).await
            .and(writer.write_all(&[SP]).await
            .and(writer.write_all(data.as_bytes()).await
            .and(writer.write_all(&[CR, LF]).await
        ))).or(Err(WriteError::NetworkError))
    }
}

struct ResponderInner<'a, 'client, C: Read + Write> {
    status: StatusCode,
    server: ResponseHeader<'a>,
    client: &'client mut C,
}

impl<'a, 'client, C: Read + Write> ResponderInner<'a, 'client, C> {
    #[must_use = "http responder not finished with either `with_body` or `no_body` results in a client waiting for data"]
    async fn with_status(&mut self, status: StatusCode) -> Result<(), ResponderError> {
        status
            .write(self.client)
            .await
            .map_err(<WriteError as core::convert::Into<ResponderError>>::into)?;

        if ResponseHeader::Server("") != self.server {
            self.server
                .write(self.client)
                .await
                .map_err(<WriteError as core::convert::Into<ResponderError>>::into)?;
        }

        Ok(())
    }

    #[must_use = "http responder not finished with either `with_body` or `no_body` results in a client waiting for data"]
    async fn with_header(&mut self, header: ResponseHeader<'a>) -> Result<(), ResponderError> {
        header.write(self.client).await?;

        Ok(())
    }

    async fn no_body(self) -> Result<(), ResponderError> {
        self.client
            .write_all(&[CR, LF])
            .await
            .or(Err(ResponderError::NetworkError))?;

        Ok(())
    }

    async fn with_body(self, body: &[u8]) -> Result<(), ResponderError> {
        ResponseHeader::ContentLength(body.len())
            .write(self.client)
            .await?;

        self.client
            .write_all(&[CR, LF])
            .await
            .or(Err(ResponderError::NetworkError))?;

        if self.client.write_all(body).await.is_err() {
            return Err(ResponderError::NetworkError);
        }

        Ok(())
    }

    async fn websocket(self) -> Result<Websocket<'client, C>, ResponderError> {
        self.client
            .write_all(&[CR, LF])
            .await
            .or(Err(ResponderError::NetworkError))?;

        Ok(Websocket::new(self.client))
    }
}

/// Responder is the API provided to formulate HTTP responses to the client. A `Responder`
/// will transition to the sending state (`ResponderSending`) when a status is sent.
pub struct Responder<'a, 'client, C: Read + Write> {
    inner: ResponderInner<'a, 'client, C>,
}

impl<'a, 'client, C: Read + Write> Responder<'a, 'client, C> {
    /// Create a new responder.  The initial status is set to 200 OK which will be sent to the
    /// client if the user skips straight to sending a header.  The responder sents the Server
    /// header to the value of the Host header in the request.
    pub fn new(request: &Request<'a>, client: &'client mut C) -> Self {
        Self {
            inner: ResponderInner {
                client,
                status: StatusCode::OK,
                server: ResponseHeader::Server(request.host),
            },
        }
    }

    /// Set and send the provided status to the client.  Consumes the `self` and returns a new self
    /// that is in the Sending state.
    #[must_use = "http responder not finished with either `with_body` or `no_body` results in a client waiting for data"]
    pub async fn with_status(
        mut self,
        status: StatusCode,
    ) -> Result<ResponderSending<'a, 'client, C>, ResponderError> {
        self.inner.with_status(status).await?;

        Ok(ResponderSending { inner: self.inner })
    }

    #[must_use = "http responder not finished with either `with_body` or `no_body` results in a client waiting for data"]
    /// Sends the supplied header to the client.  Consumes
    /// the self returning a Self in the Sending state.
    pub async fn with_header(
        mut self,
        header: ResponseHeader<'a>,
    ) -> Result<ResponderSending<'a, 'client, C>, ResponderError> {
        self.inner.with_status(self.inner.status).await?;

        header.write(self.inner.client).await?;

        Ok(ResponderSending { inner: self.inner })
    }

    /// Upgrade the client to a Websocket.  Consumees the self and returns a Websocket, or an error
    /// if the request doesn not contain, or contains an invalid Sec-Websocket-Key header value.
    pub async fn upgrade(
        mut self,
        req: Request<'a>,
    ) -> Result<Websocket<'client, C>, ResponderError> {
        let websocket_key = match req.get_header(RequestHeader::SecWebSocketKey("")) {
            Some(RequestHeader::SecWebSocketKey(k)) => k,
            _ => {
                self.inner.with_status(StatusCode::BadRequest).await?;
                self.inner.no_body().await?;

                return Err(ResponderError::ProtocolError(
                    "websocket upgrade did not include a Sec-Websocket-Key header",
                ));
            }
        };

        let accept_key = match sec_websocket_accept_val(websocket_key) {
            Ok(k) => k,
            Err(e) => {
                self.inner.with_status(StatusCode::BadRequest).await?;
                self.inner.no_body().await?;

                return Err(ResponderError::ProtocolError(e));
            }
        };

        self.inner
            .with_status(StatusCode::SwitchingProtocols)
            .await?;
        self.inner
            .with_header(ResponseHeader::SecWebSocketAccept(accept_key))
            .await?;
        self.inner
            .with_header(ResponseHeader::Other("Upgrade", "websocket"))
            .await?;
        self.inner
            .with_header(ResponseHeader::Connection("Upgrade"))
            .await?;
        self.inner.websocket().await
    }
}

/// ResponderSending is a responder for which a status has already been sent
pub struct ResponderSending<'a, 'client, C: Read + Write> {
    inner: ResponderInner<'a, 'client, C>,
}

impl<'a, 'client, C: Read + Write> ResponderSending<'a, 'client, C> {
    #[must_use = "http responder not finished with either `with_body` or `no_body` results in a client waiting for data"]
    /// Sends the supplied header to the client.  Consumes
    /// the self returning a Self in the Sending state.
    pub async fn with_header(
        self,
        header: ResponseHeader<'a>,
    ) -> Result<ResponderSending<'a, 'client, C>, ResponderError> {
        header.write(self.inner.client).await?;

        Ok(self)
    }

    /// Completes the response with no body.  Comsumes the self as it is not valid to produce any
    /// more data to the client in response to the active request.
    pub async fn no_body(self) -> Result<(), ResponderError> {
        self.inner.no_body().await
    }

    /// Completes the response with the supplied body setting the Content-Length to the length of the body.
    /// Comsumes the self as it is not valid to produce any more data to the client in response to the active request.
    pub async fn with_body(self, body: &[u8]) -> Result<(), ResponderError> {
        self.inner.with_body(body).await
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use embedded_io_async::{ErrorKind, ErrorType};
    use std::vec::Vec;
    use std::*;

    use crate::request::Method;

    use super::*;

    struct TestClient<'a> {
        inner: &'a mut Vec<u8>,
    }

    impl<'a> TestClient<'a> {
        fn new(inner: &'a mut Vec<u8>) -> Self {
            Self { inner: inner }
        }
    }

    impl<'a> ErrorType for TestClient<'a> {
        type Error = ErrorKind;
    }

    impl<'a> Write for TestClient<'a> {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.inner.extend_from_slice(buf);
            Ok(buf.len())
        }

        async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            self.inner.extend_from_slice(buf);
            Ok(())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl<'a> Read for TestClient<'a> {
        async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }
    }

    // HTTP uses `\r\n` as EOL delimeters.  In the expected data, we manually add
    // the \r at the end of the line, before the inherrent \n.

    #[tokio::test]
    async fn test_http_response_default() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        // let resp = HttpResponse::<3>::new();
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        let expected = "HTTP/1.1 200 OK\r
Server: RustServer\r
Content-Type: text/html\r
\r
"
        .as_bytes();

        resp.with_status(StatusCode::OK)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_default_with_body() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        // let resp = HttpResponse::<3>::new();
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        let body = "<html>
    <head>
        <title>Testing</title>
    </head>
    <body>
        <p>works!</p>
    </body>
</html>
"
        .as_bytes();

        resp.with_status(StatusCode::OK)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .with_body(body)
            .await
            .unwrap();

        let expected = "HTTP/1.1 200 OK\r
Server: RustServer\r
Content-Type: text/html\r
Content-Length: 114\r
\r
<html>
    <head>
        <title>Testing</title>
    </head>
    <body>
        <p>works!</p>
    </body>
</html>
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_with_status() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        resp.with_status(StatusCode::NotFound)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        let expected = "HTTP/1.1 404 Not Found\r
Server: RustServer\r
Content-Type: text/html\r
\r
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_with_custom_status() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        resp.with_status(StatusCode::Other(401))
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        let expected = "HTTP/1.1 401\r
Server: RustServer\r
Content-Type: text/html\r
\r
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_with_custom_content_type() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        resp.with_status(StatusCode::OK)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("application/json"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        let expected = "HTTP/1.1 200 OK\r
Server: RustServer\r
Content-Type: application/json\r
\r
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_with_custom_server() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "FancyServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        resp.with_status(StatusCode::OK)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        let expected = "HTTP/1.1 200 OK\r
Server: FancyServer\r
Content-Type: text/html\r
\r
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_with_one_extra_header() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        resp.with_status(StatusCode::OK)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .with_header(ResponseHeader::Other("Foo", "Bar"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        let expected = "HTTP/1.1 200 OK\r
Server: RustServer\r
Content-Type: text/html\r
Foo: Bar\r
\r
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }

    #[tokio::test]
    async fn test_http_response_with_multiple_extra_header() {
        let request = Request::<'_> {
            method: Method::GET,
            path: "/",
            host: "RustServer",
            content_type: None,
            user_agent: None,
            content_length: 0,
            body: None,
            header_slice: None,
        };

        let mut dst = Vec::<u8>::new();
        let mut writer = TestClient::new(&mut dst);
        let resp = Responder::<'_, '_, TestClient>::new(&request, &mut writer);

        resp.with_status(StatusCode::OK)
            .await
            .unwrap()
            .with_header(ResponseHeader::ContentType("text/html"))
            .await
            .unwrap()
            .with_header(ResponseHeader::Other("Foo-One", "Bar"))
            .await
            .unwrap()
            .with_header(ResponseHeader::Other("Foo-Two", "Baz"))
            .await
            .unwrap()
            .with_header(ResponseHeader::Other("Foo-Three", "Bat"))
            .await
            .unwrap()
            .no_body()
            .await
            .unwrap();

        let expected = "HTTP/1.1 200 OK\r
Server: RustServer\r
Content-Type: text/html\r
Foo-One: Bar\r
Foo-Two: Baz\r
Foo-Three: Bat\r
\r
"
        .as_bytes();

        assert_eq!(
            &dst,
            expected,
            "oops, got:\n{}",
            str::from_utf8(&dst).unwrap()
        );
    }
}
