use embedded_io_async::{Read, Write};

use crate::request::{Request, RequestError};
use crate::response::{Responder, ResponderError};
use crate::websocket::{Websocket, WebsocketError};

/// HandlerError is returned by `RequestHandler` implementations.  Errors returned by `Responder`
/// method should be passed up in the `ResponderError` variant, other errors are a `CustomError`
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HandlerError {
    /// Errors returned by `Responder` methods
    ResponderError(ResponderError),
    /// Errors returned from websocket operations
    WebsocketError(WebsocketError),
    /// Custom errors as specified by the `RequestHandler` implementation author
    CustomError(&'static str),
}

impl From<ResponderError> for HandlerError {
    fn from(value: ResponderError) -> Self {
        Self::ResponderError(value)
    }
}

impl From<WebsocketError> for HandlerError {
    fn from(value: WebsocketError) -> Self {
        Self::WebsocketError(value)
    }
}

impl From<&'static str> for HandlerError {
    fn from(value: &'static str) -> Self {
        Self::CustomError(value)
    }
}

/// Trait required to be implemeted be the resource that will be responsible for handling requests.
pub trait RequestHandler {
    /// Called by the server passing a Request, and Responder.  The implementation should
    /// use the Responder to generate the appropriate HTTP response for the client.
    /// If a Ok(Some(Websocket)) is returned, the server will subsequently call the
    /// handle_websocket method.
    ///
    /// ```
    /// use embedded_io_async::{Read, Write};
    ///
    /// use httplite::request::Request;
    /// use httplite::response::{Responder, StatusCode};
    /// use httplite::websocket::Websocket;
    /// use httplite::server::{RequestHandler, HandlerError, Server};
    ///
    /// struct Handler {}
    ///
    /// impl RequestHandler for Handler {
    ///     async fn handle_request<'client, 'buff, C: Read + Write + 'client>(
    ///         &self,
    ///         req: Request<'buff>,
    ///         resp: Responder<'buff, 'client, C>
    ///     ) -> Result<Option<Websocket<'client, C>>, HandlerError> {
    ///         match req.path {
    ///             "/" => {
    ///                 resp.with_status(StatusCode::OK)
    ///                     .await?
    ///                     .with_body(b"<html>...")
    ///                     .await?;
    ///             },
    ///             _ => {
    ///                 resp.with_status(StatusCode::NotFound)
    ///                     .await?
    ///                     .with_body(b"not found")
    ///                     .await?;
    ///             }
    ///         }
    ///
    ///         Ok(None)
    ///     }
    /// }
    fn handle_request<'client, 'buff, C: Read + Write + 'client>(
        &self,
        req: Request<'buff>,
        resp: Responder<'buff, 'client, C>,
    ) -> impl Future<Output = Result<Option<Websocket<'client, C>>, HandlerError>>;

    /// Called by server if the call to handle_rquest returns a Ok(Some(Websocket)), passing the
    /// websocket and the http_buffer.  The implementation should handle incomming websocket frames
    /// and generate outbound frames for the duration of the connection.
    ///
    /// ```
    /// use embedded_io_async::{Read, Write};
    ///
    /// use httplite::request::Request;
    /// use httplite::response::{Responder, StatusCode};
    /// use httplite::websocket::Websocket;
    /// use httplite::server::{RequestHandler, HandlerError, Server};
    ///
    /// struct Handler {}
    ///
    /// impl RequestHandler for Handler {
    /// #    async fn handle_request<'client, 'buff, C: Read + Write + 'client>(
    /// #        &self,
    /// #        req: Request<'buff>,
    /// #        resp: Responder<'buff, 'client, C>
    /// #    ) -> Result<Option<Websocket<'client, C>>, HandlerError> {
    /// #        Err(HandlerError::CustomError("not implemented"))
    /// #    }
    ///
    ///     async fn handle_websocket<'client, C: Read + Write + 'client>(
    ///         &self,
    ///         mut websocket: Websocket<'client, C>,
    ///         buffer: &mut [u8],
    ///     ) -> Result<(), HandlerError> {
    ///         let mut pong = *b"pong";
    ///
    ///         loop {
    ///             let ping = websocket.receive(buffer).await.unwrap();
    ///             websocket.send(&mut pong[..]).await.unwrap();
    ///         }
    ///         Ok(())
    ///     }
    /// }
    fn handle_websocket<'client, C: Read + Write + 'client>(
        &self,
        mut _websocket: Websocket<'client, C>,
        _buffer: &mut [u8],
    ) -> impl Future<Output = Result<(), HandlerError>> {
        async { Err(HandlerError::CustomError("websocket not implemented")) }
    }
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// ServerError is returned by the httplite::server::Server:serve() method when any of the variants
/// occur.
pub enum ServerError {
    /// Error parsing HTTP or Websocket protocol data
    ProtocolError(&'static str),
    /// Incomming data (request body or websocket payload) that exceeded the size of the provided
    /// buffer.  The value within the BufferExceeded indicates the incoming data size.
    BufferExceeded(u64),
    /// Error returned by handler
    HandlerError(HandlerError),
}

/// Server is the main struct to be used by users of the crate.  It is constructed with an
/// implementation of RequestHandler, provides a serve() method to be called on each new client
/// connection.
pub struct Server<H> {
    handler: H,
}

impl<H> Server<H>
where
    H: RequestHandler,
{
    /// Construct an Server using the provided implementation of RequestHandler
    pub fn new(handler: H) -> Self {
        Self { handler }
    }

    /// process requests from the client, calling the provided RequestHandler with the request and
    /// a Responder.  The result will be `OK(())` when the client disconnects.  The result will be
    /// an Err if there is a HTTP protocol error, or if a request is received that exceeds the
    /// buffer size as indicated by the request Content-Length.   If a client sends more data than
    /// is indicated by the Content-Length, then *Content-Length* bytes will be read as the body of
    /// the current request, and then subsequent bytes will be read as the next request likely
    /// resulting in a protocol error being returned.  Any Err(_) variant should be handled by
    /// disconnecting the client.
    pub async fn serve<C>(&self, client: &mut C, http_buff: &mut [u8]) -> Result<(), ServerError>
    where
        C: Read + Write,
    {
        loop {
            let mut http_buff_offset = 0;
            loop {
                let res = client.read(&mut http_buff[http_buff_offset..]).await;
                match res {
                    Ok(0) => {
                        if http_buff_offset > 0 {
                            // we filled the buffer while outstanding request data remains
                            return Err(ServerError::BufferExceeded(0));
                        }
                        return Ok(());
                    }
                    Ok(n) => {
                        http_buff_offset += n;
                        match Request::parse(&http_buff[..]) {
                            Ok(request) => {
                                // handle request for response
                                let resp = Responder::<'_, '_, _>::new(&request, client);
                                if let Err(e) =
                                    match self.handler.handle_request(request, resp).await {
                                        Ok(None) => break,
                                        Ok(Some(ws)) => {
                                            self.handler.handle_websocket(ws, http_buff).await
                                        }
                                        Err(e) => Err(e),
                                    }
                                {
                                    match e {
                                        HandlerError::ResponderError(
                                            ResponderError::NetworkError,
                                        ) => return Ok(()),
                                        HandlerError::ResponderError(
                                            ResponderError::ProtocolError(s),
                                        ) => return Err(ServerError::ProtocolError(s)),
                                        _ => return Err(ServerError::HandlerError(e)),
                                    }
                                }
                            }
                            Err(RequestError::ProtocolError(e)) => {
                                return Err(ServerError::ProtocolError(e));
                            }
                            Err(RequestError::Incomplete(_)) => continue,
                        };
                    }
                    Err(_) => return Ok(()),
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;
    use std::*;

    use embedded_io_async::{ErrorKind, ErrorType};

    use super::*;
    use crate::response::StatusCode;
    use crate::websocket::Websocket;

    struct TestReader<'a> {
        max_reads: usize,
        reads: usize,
        inner: &'a mut Vec<u8>,
    }

    impl<'a> TestReader<'a> {
        fn new(inner: &'a mut Vec<u8>, max_reads: usize) -> Self {
            Self {
                inner,
                max_reads,
                reads: 0,
            }
        }
    }

    impl<'a> ErrorType for TestReader<'a> {
        type Error = ErrorKind;
    }

    impl<'a> Read for TestReader<'a> {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            if self.reads >= self.max_reads {
                return Err(Self::Error::ConnectionReset);
            }
            self.reads += 1;

            if self.inner.len() > buf.len() {
                buf.copy_from_slice(&self.inner[..buf.len()]);
                return Ok(buf.len());
            }

            buf[..self.inner.len()].copy_from_slice(&self.inner[..]);
            Ok(self.inner.len())
        }
    }

    struct TestWriter<'a> {
        inner: &'a mut Vec<u8>,
    }

    impl<'a> TestWriter<'a> {
        fn new(inner: &'a mut Vec<u8>) -> Self {
            Self { inner: inner }
        }
    }

    impl<'a> ErrorType for TestWriter<'a> {
        type Error = ErrorKind;
    }

    impl<'a> Write for TestWriter<'a> {
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

    struct TestReaderWriter<'a> {
        reader: TestReader<'a>,
        writer: TestWriter<'a>,
    }

    impl<'a> ErrorType for TestReaderWriter<'a> {
        type Error = ErrorKind;
    }

    impl<'a> Read for TestReaderWriter<'a> {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            self.reader.read(buf).await
        }
    }

    impl<'a> Write for TestReaderWriter<'a> {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.writer.write(buf).await
        }

        async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            self.writer.inner.extend_from_slice(buf);
            Ok(())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            self.writer.flush().await
        }
    }

    struct Handler {}
    impl RequestHandler for Handler {
        async fn handle_request<'buff, 'client, C: Read + Write + 'client>(
            &self,
            req: Request<'buff>,
            resp: Responder<'buff, 'client, C>,
        ) -> Result<Option<Websocket<'client, C>>, HandlerError> {
            match req.path {
                "/index.html" => {
                    resp.with_status(StatusCode::OK)
                        .await?
                        .with_body("working".as_bytes())
                        .await?
                }
                "/test1" => {
                    resp.with_status(StatusCode::OK)
                        .await?
                        .with_body("test1".as_bytes())
                        .await?
                }
                _ => {
                    resp.with_status(StatusCode::NotFound)
                        .await?
                        .with_body("Not Found".as_bytes())
                        .await?
                }
            }
            Ok(None)
        }
    }

    #[tokio::test]
    async fn test_http_server() {
        let handler = Handler {};
        let server = Server::<Handler>::new(handler);

        let mut reader_buf = "GET /index.html HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc"
            .as_bytes()
            .to_vec();
        let mut writer_buf = Vec::<u8>::new();

        let mut client = TestReaderWriter {
            reader: TestReader::new(&mut reader_buf, 1),
            writer: TestWriter::new(&mut writer_buf),
        };

        let mut http_buff = [0u8; 2048];

        match server.serve(&mut client, &mut http_buff[..]).await {
            Ok(_) => {}
            Err(e) => {
                std::panic!("{:?}", e);
            }
        }

        assert_eq!(
            writer_buf.as_slice(),
            "HTTP/1.1 200 OK\r
Content-Length: 7\r
\r
working"
                .as_bytes()
        );
    }
}
