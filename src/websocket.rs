//! A websocket is an  connection that is "upgraded" to a Websocket.  Which basically implies
//! taking the underlying client connection (e.g. TCP connection) and re-purposing to carry a
//! Websocket session.  Once upgraded, it cannot be downgraded.
//! The websocket protocol is a bidirectional non-syncronous exchange of WebSockt frames that
//! encapsulate payload data.
//!
//! For more info:
//!
//! * <https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API/Writing_WebSocket_servers>
//!
//! A Websocket should be obtained by upgrading an  sesison via the Responder.
//!
//! ```
//! use embedded_io_async::{Read, Write};
//!
//! use httplite::request::Request;
//! use httplite::response::{Responder, StatusCode};
//! use httplite::websocket::Websocket;
//! use httplite::server::{RequestHandler, HandlerError, Server};
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
//!             "/ws" => {
//!                 // upgrade and return the websocket, which will then be
//!                 // used to call handle_websocket
//!                 let ws = resp.upgrade(req).await?;
//!                 return Ok(Some(ws));
//!             }
//!             _ => {
//!                 resp.with_status(StatusCode::NotFound).await?.no_body().await?;
//!             }
//!         }
//!
//!         Ok(None)
//!     }
//!
//!     async fn handle_websocket<'client, C: Read + Write + 'client>(
//!         &self,
//!         mut websocket: Websocket<'client, C>,
//!         buffer: &mut [u8],
//!     ) -> Result<(), HandlerError> {
//!         let mut ping = b"ping";
//!         let mut pong = *b"pong";
//!
//!         loop {
//!             let frame  = websocket.receive(buffer).await.unwrap();
//!             if buffer[..frame.len] == ping[..] {
//!                 websocket.send(&mut pong[..]).await.unwrap();
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//!
//! ```

use base64ct::{Base64, Encoding};
use embedded_io_async::{Read, Write};
use sha1::{Digest, Sha1};

const SEC_WEBSOCKET_ACCEPT_MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub(crate) fn sec_websocket_accept_val(key: &str) -> Result<[u8; 28], &'static str> {
    let mut key_hasher = Sha1::new();
    key_hasher.update(key.as_bytes());
    key_hasher.update(SEC_WEBSOCKET_ACCEPT_MAGIC.as_bytes());
    let key_hash = key_hasher.finalize();

    let mut key_b64_buff = [0u8; 28];
    if Base64::encode(&key_hash, &mut key_b64_buff).is_err() {
        return Err("error enoding key hash due to invalid length");
    }

    Ok(key_b64_buff)
}

/// WebsocketError contains the errors that may be returned while handling a websocket connection.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum WebsocketError {
    /// Indicates that the received payload is smaller than the frame indicated
    InsufficientData(usize),
    /// Unsupported indicates that the incoming payload size exceeds the receive buffer
    Unsupported(&'static str),
    /// Network Error during a read or write with the client
    NetworkError,
}

/// Provides the Websocket protocol over the client connection
pub struct Websocket<'a, C: Read + Write> {
    conn: &'a mut C,
}

impl<'a, C: Read + Write> Websocket<'a, C> {
    /// Return a new Websocket over the provided cllient connection
    pub fn new(conn: &'a mut C) -> Self {
        Self { conn }
    }

    /// Receive a websocket frame from the client writing the payload data into the supplied buffer.
    /// Returns a WebsocketFrame or an error where encountered.  The caller should check that the
    /// OP code reported in the frame is according to their logic, and use the length field of the
    /// WebsocketFrame to know how much was written into the buffer.
    pub async fn receive(&mut self, buf: &mut [u8]) -> Result<WebsocketFrame, WebsocketError> {
        let mut offset = 0;
        let mut header_buf = [0u8; 14];

        self.conn
            .read_exact(&mut header_buf[..6])
            .await
            .map_err(|_| WebsocketError::NetworkError)?;
        offset += 6;

        let header: WebsocketFrame;
        loop {
            header = match WebsocketFrame::decode(&header_buf[..offset]) {
                Ok(h) => h,
                Err(WebsocketError::InsufficientData(n)) => {
                    self.conn
                        .read_exact(&mut header_buf[offset..offset + n])
                        .await
                        .map_err(|_| WebsocketError::NetworkError)?;
                    offset += n;
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };
            break;
        }

        if header.len > buf.len() {
            return Err(WebsocketError::Unsupported(
                "payload length exceeds buffer size",
            ));
        }

        self.conn
            .read_exact(&mut buf[..header.len])
            .await
            .map_err(|_| WebsocketError::NetworkError)?;

        if header.masked {
            header.apply_mask(&mut buf[..header.len]);
        }

        Ok(header)
    }

    /// Send the provided data bytes to the client after encoding it into a Websocket frame
    pub async fn send(&mut self, data: &mut [u8]) -> Result<(), WebsocketError> {
        let header = WebsocketFrame {
            fin: true,
            opcode: 2,
            masked: false,
            len: data.len(),
            mask: None,
        };

        let mut encoded_header = [0u8; 14];
        let header_len = header.encode(&mut encoded_header)?;

        self.conn
            .write_all(&encoded_header[..header_len])
            .await
            .map_err(|_| WebsocketError::NetworkError)?;

        self.conn
            .write_all(data)
            .await
            .map_err(|_| WebsocketError::NetworkError)?;

        Ok(())
    }
}

/// WebsocketFrame encodes/decodes to the websocket wire protocol
#[derive(Debug)]
pub struct WebsocketFrame {
    /// The websocket OP code value
    pub opcode: u8,
    /// The length of the payload
    pub len: usize,
    fin: bool,
    masked: bool,
    mask: Option<[u8; 4]>,
}

impl WebsocketFrame {
    fn decode(value: &[u8]) -> Result<Self, WebsocketError> {
        let mut required_bytes = 2usize;

        if value.len() < required_bytes {
            return Err(WebsocketError::InsufficientData(
                required_bytes - value.len(),
            ));
        }

        let fin: bool = (value[0] & 128) == 128;
        let opcode: u8 = value[0] & 0x0F;

        if !fin || opcode == 0 {
            return Err(WebsocketError::Unsupported(
                "payload fragmentation not supported",
            ));
        }

        let masked: bool = (value[1] & 128) == 128;

        let mut len: u64 = (value[1] << 1 >> 1) as u64;
        let mut mask_offset = 2;
        if len == 126 {
            // 16 bit length field
            required_bytes += 2;
            if value.len() < required_bytes {
                return Err(WebsocketError::InsufficientData(
                    required_bytes - value.len(),
                ));
            }
            len = (value[2] as u64) << 8 | value[3] as u64;
            mask_offset = 4;
        }
        if len == 127 {
            // 64bit length field
            required_bytes += 8;
            if value.len() < required_bytes {
                return Err(WebsocketError::InsufficientData(
                    required_bytes - value.len(),
                ));
            }
            len = (value[2] as u64) << 56
                | (value[3] as u64) << 48
                | (value[4] as u64) << 40
                | (value[5] as u64) << 32
                | (value[6] as u64) << 24
                | (value[7] as u64) << 16
                | (value[8] as u64) << 8
                | value[9] as u64;
            mask_offset = 10;
        }

        let len: usize = match usize::try_from(len) {
            Ok(l) => l,
            Err(_) => {
                return Err(WebsocketError::Unsupported(
                    "payload length exceeds max platform architecture usize",
                ));
            }
        };

        let mut mask: Option<[u8; 4]> = None;

        if masked {
            required_bytes += 4;
            if value.len() < required_bytes {
                return Err(WebsocketError::InsufficientData(
                    required_bytes - value.len(),
                ));
            }

            mask = Some(value[mask_offset..mask_offset + 4].try_into().unwrap());
        }

        Ok(WebsocketFrame {
            fin,
            opcode,
            masked,
            len,
            mask,
        })
    }

    fn encode(&self, dest: &mut [u8]) -> Result<usize, WebsocketError> {
        if dest.len() < 2 {
            return Err(WebsocketError::Unsupported(
                "encode buffer requires at least 6 bytes",
            ));
        }

        // fin 1 MSB byte 1
        if self.fin {
            dest[0] ^= 0b1000_0000;
        }

        // opcode 4 LSB bits byte 1
        dest[0] ^= self.opcode & 0b000_1111;

        // masked 1 MSB byte 2
        if self.masked {
            dest[1] ^= 0b1000_0000;
        }

        let mut mask_offset = 2;
        if self.len <= 125 {
            if dest.len() < 2 {
                return Err(WebsocketError::Unsupported(
                    "encode buffer requires at least 6 bytes for given payload lenght",
                ));
            }
            // 7 LSB byte 2
            dest[1] ^= self.len as u8;
        }

        if self.len > 125 && self.len <= u16::MAX.into() {
            if dest.len() < 4 {
                return Err(WebsocketError::Unsupported(
                    "encode buffer requires at least 8 bytes for given payload lenght",
                ));
            }

            // indicate 16 bit length with byte 2 7LSB bits = 126
            dest[1] ^= 126u8;
            [dest[2], dest[3]] = (self.len as u16).to_be_bytes();
            mask_offset = 4;
        }

        if self.len > u16::MAX.into() {
            if dest.len() < 10 {
                return Err(WebsocketError::Unsupported(
                    "encode buffer requires at least 14 bytes for given payload lenght",
                ));
            }

            // indicate 64 bit length with byte 2 7LSB bits = 127
            dest[1] ^= 127u8;
            [
                dest[2], dest[3], dest[4], dest[5], dest[6], dest[7], dest[8], dest[9],
            ] = (self.len as u64).to_be_bytes();
            mask_offset = 10;
        }

        if let Some(mask) = self.mask {
            dest[mask_offset] ^= mask[0];
            dest[mask_offset + 1] ^= mask[1];
            dest[mask_offset + 2] ^= mask[2];
            dest[mask_offset + 3] ^= mask[3];

            return Ok(mask_offset + 4);
        }

        Ok(mask_offset)
    }

    fn apply_mask(&self, data: &mut [u8]) {
        if let Some(mask) = self.mask {
            for i in 0..self.len {
                data[i] ^= mask[i % 4];
            }
        }
    }
}
