//! # Weblite
//!
//! `weblite` is a **very** basic implementation of the HTTP protocol predominantly aimed at
//! `no_std` and `no_alloc` use cases such as embedded development.
//!
//! This crate provides:
//!
//! * encoding and decoding of HTTP requests and responses on the "wire" respectively.
//! * encoding and decoding of websocket frames on the "wire".
//!
//! This crate does **not** provide:
//!
//! * any mechanism for routing requests to specific handlers.
//! * any higher level functionality for extracting data from paths, or request bodies.
//!
//! ## Basic Use
//!
//! Start by creating a `server::Server` passing it a resource that implements the `server::RequestHandler`
//! trait.  When a client connects on a TCP socket (or anything that implements
//! `embedded_io_async::{Read, Write}`), call `serve()` on the `Server` passing the "socket"
//! and a `&mut [u8]` buffer that will be used to read `Request` data into.  The buffer should
//! be large enough to receive any anticipated request including bodies.  If using websockets, the
//! buffer should also be large enough to hold any anticipated incoming frame.
//!
//! ## Example
//!
//! ```
//! # use tokio;
//! use embedded_io_async::{Read, Write};
//!
//! use weblite::request::Request;
//! use weblite::response::{Responder, StatusCode};
//! use weblite::websocket::Websocket;
//! use weblite::server::{RequestHandler, HandlerError, Server};
//!
//! const HTML_INDEX: &str = "<html>...</html>";
//! const HTML_404: &str = "Not Found";
//!
//! struct MyHandler;
//!
//! impl RequestHandler for MyHandler {
//!     async fn handle_request<'client, 'buff, C: Read + Write + 'client>(
//!         &self,
//!         req: Request<'buff>,
//!         resp: Responder<'buff, 'client, C>,
//!     ) -> Result<Option<Websocket<'client, C>>, HandlerError> {
//!         match req.path {
//!            "/" => {
//!                resp.with_status(StatusCode::OK)
//!                    .await?
//!                    .with_body(HTML_INDEX.as_bytes())
//!                    .await?;
//!            }
//!            _ => {
//!                resp.with_status(StatusCode::NotFound)
//!                    .await?
//!                    .with_body(HTML_404.as_bytes())
//!                    .await?;
//!            }
//!         }
//!
//!         Ok(None)
//!     }
//! }
//!
//! # struct Client<'a> {
//! #     reader: &'a [u8],
//! #     writer: &'a mut[u8],
//! # }
//! #
//! # impl<'a> embedded_io_async::ErrorType for Client<'a> {
//! #     type Error = embedded_io_async::ErrorKind;
//! # }
//! #
//! # impl<'a> embedded_io_async::Read for Client<'a> {
//! #     async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
//! #         match self.reader.read(buf).await {
//! #             Ok(n) => Ok(n),
//! #             Err(_) => Err(embedded_io_async::ErrorKind::Other),
//! #         }
//! #     }
//! # }
//! #
//! # impl<'a> embedded_io_async::Write for Client<'a> {
//! #     async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
//! #         match self.writer.write(buf).await {
//! #             Ok(n) => Ok(n),
//! #             Err(_) => Err(embedded_io_async::ErrorKind::Other),
//! #         }
//! #     }
//! # }
//! #
//! async fn run_server() {
//!     let mut read_buf = [0u8;4096];
//!     let mut write_buf = [0u8;4096];
//!
//!     // Client implements embedded_io_async::{Read, Write} (not shown)
//!     // this would typically be an implementation of a TCP Socket that implements the traits.
//!     // e.g. embassy_net::tcp::TcpSocket
//!     let mut client = Client{
//!         reader: read_buf.as_slice(),
//!         writer: write_buf.as_mut_slice(),
//!     };
//!
//!     let handler = MyHandler;
//!     let server = Server::new(handler);
//!
//!     let mut http_buffer = [0u8;2048];
//!     if server.serve(
//!         &mut client,
//!         &mut http_buffer[..],
//!     ).await.is_err() {
//!         // handle error
//!     }
//! }
//! #
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! #     run_server().await;
//! # })
//! ```

#![no_std]
#![warn(missing_docs)]

mod ascii;
/// HTTP Headers
pub mod header;
/// HTTP Requests
pub mod request;
/// HTTP responses
pub mod response;
/// HTTP server
pub mod server;
/// Websockets
pub mod websocket;

use embedded_io_async::Write;

pub(crate) enum WriteError {
    NetworkError,
}

pub(crate) trait HttpWrite {
    async fn write<T: Write>(self, writer: &mut T) -> Result<(), WriteError>;
}
