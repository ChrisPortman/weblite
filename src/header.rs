use embedded_io_async::Write;

use crate::ascii::{AsciiInt, CR, LF, atoi};
use crate::{HttpWrite, WriteError};

/// Host
pub const REQ_HEAD_HOST: &str = "Host";
/// User-Agent
pub const REQ_HEAD_USER_AGENT: &str = "User-Agent";
/// Upgrade
pub const REQ_HEAD_UPGRADE: &str = "Upgrade";
/// Sec-WebSocket-Key
pub const REQ_HEAD_SEC_WEBSOCKET_KEY: &str = "Sec-WebSocket-Key";
/// Accept
pub const REQ_HEAD_ACCEPT: &str = "Accept";
/// Accept-Language
pub const REQ_HEAD_ACCEPT_LANGUAGE: &str = "Accept-Language";
/// Accept-Encoding
pub const REQ_HEAD_ACCEPT_ENCODING: &str = "Accept-Encoding";
/// Referer
pub const REQ_HEAD_REFERER: &str = "Referer";
/// Connection
pub const REQ_HEAD_CONNECTION: &str = "Connection";
/// Upgrade-Insecure-Requests
pub const REQ_HEAD_UPGRADE_INSECURE_REQUESTS: &str = "Upgrade-Insecure-Requests";
/// If-Modified-Since
pub const REQ_HEAD_IF_MODIFIED_SINCE: &str = "If-Modified-Since";
/// If-None-Match
pub const REQ_HEAD_IF_NONE_MATCH: &str = "If-None-Match";
/// Cache-Control
pub const REQ_HEAD_CACHE_CONTROL: &str = "Cache-Control";
/// Content-Length
pub const REQ_HEAD_CONTENT_LENGTH: &str = "Content-Length";
/// Content-Range
pub const REQ_HEAD_CONTENT_RANGE: &str = "Content-Range";
/// Content-Type
pub const REQ_HEAD_CONTENT_TYPE: &str = "Content-Type";
/// Content-Encoding
pub const REQ_HEAD_CONTENT_ENCODING: &str = "Content-Encoding";
/// Content-Location
pub const REQ_HEAD_CONTENT_LOCATION: &str = "Content-Location";
/// Content-Language
pub const REQ_HEAD_CONTENT_LANGUAGE: &str = "Content-Language";
/// ETag
pub const REQ_HEAD_ETAG: &str = "ETag";

#[allow(missing_docs)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RequestHeader<'a> {
    Host(&'a str),
    UserAgent(&'a str),
    Upgrade(&'a str),
    SecWebSocketKey(&'a str),
    Accept(&'a str),
    AcceptLanguage(&'a str),
    AcceptEncoding(&'a str),
    Referer(&'a str),
    Connection(&'a str),
    UpgradeInsecureRequests(&'a str),
    IfModifiedSince(&'a str),
    IfNoneMatch(&'a str),
    CacheControl(&'a str),
    ContentLength(usize),
    ContentRange(&'a str),
    ContentType(&'a str),
    ContentEncoding(&'a str),
    ContentLocation(&'a str),
    ContentLanguage(&'a str),
    ETag(&'a str),
    Other(&'a str, &'a str),
}

impl<'a> TryFrom<(&'a str, &'a str)> for RequestHeader<'a> {
    type Error = Option<&'static str>;

    fn try_from(value: (&'a str, &'a str)) -> Result<Self, Self::Error> {
        match value.0 {
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_HOST) => Ok(RequestHeader::Host(value.1)),
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_USER_AGENT) => {
                Ok(RequestHeader::UserAgent(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_UPGRADE) => {
                Ok(RequestHeader::Upgrade(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_SEC_WEBSOCKET_KEY) => {
                Ok(RequestHeader::SecWebSocketKey(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_ACCEPT) => {
                Ok(RequestHeader::Accept(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_ACCEPT_LANGUAGE) => {
                Ok(RequestHeader::AcceptLanguage(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_ACCEPT_ENCODING) => {
                Ok(RequestHeader::AcceptEncoding(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_REFERER) => {
                Ok(RequestHeader::Referer(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONNECTION) => {
                Ok(RequestHeader::Connection(value.1))
            }
            _ if value
                .0
                .eq_ignore_ascii_case(REQ_HEAD_UPGRADE_INSECURE_REQUESTS) =>
            {
                Ok(RequestHeader::UpgradeInsecureRequests(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_IF_MODIFIED_SINCE) => {
                Ok(RequestHeader::IfModifiedSince(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_IF_NONE_MATCH) => {
                Ok(RequestHeader::IfNoneMatch(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CACHE_CONTROL) => {
                Ok(RequestHeader::CacheControl(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONTENT_RANGE) => {
                Ok(RequestHeader::ContentRange(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONTENT_TYPE) => {
                Ok(RequestHeader::ContentType(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONTENT_ENCODING) => {
                Ok(RequestHeader::ContentEncoding(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONTENT_LOCATION) => {
                Ok(RequestHeader::ContentLocation(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONTENT_LANGUAGE) => {
                Ok(RequestHeader::ContentLanguage(value.1))
            }
            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_ETAG) => Ok(RequestHeader::ETag(value.1)),

            _ if value.0.eq_ignore_ascii_case(REQ_HEAD_CONTENT_LENGTH) => {
                Ok(RequestHeader::ContentLength(
                    atoi(value.1.as_bytes()).ok_or("invalid content-length")? as usize,
                ))
            }
            _ => Ok(RequestHeader::Other(value.0, value.1)),
        }
    }
}

/// Access-Control-Allow-Origin
pub const RESP_HEAD_ACCESS_CONTROL_ALLOW_ORIGIN: &str = "Access-Control-Allow-Origin";
/// Connection
pub const RESP_HEAD_CONNECTION: &str = "Connection";
/// Date
pub const RESP_HEAD_DATE: &str = "Date";
/// Keep-Alive
pub const RESP_HEAD_KEEP_ALIVE: &str = "Keep-Alive";
/// Last-Modified
pub const RESP_HEAD_LAST_MODIFIED: &str = "Last-Modified";
/// Server
pub const RESP_HEAD_SERVER: &str = "Server";
/// Set-Cookie
pub const RESP_HEAD_SET_COOKIE: &str = "Set-Cookie";
/// Transfer-Encoding
pub const RESP_HEAD_TRANSFER_ENCODING: &str = "Transfer-Encoding";
/// Vary
pub const RESP_HEAD_VARY: &str = "Vary";
/// Content-Length
pub const RESP_HEAD_CONTENT_LENGTH: &str = "Content-Length";
/// Content-Range
pub const RESP_HEAD_CONTENT_RANGE: &str = "Content-Range";
/// Content-Type
pub const RESP_HEAD_CONTENT_TYPE: &str = "Content-Type";
/// Content-Encoding
pub const RESP_HEAD_CONTENT_ENCODING: &str = "Content-Encoding";
/// Content-Location
pub const RESP_HEAD_CONTENT_LOCATION: &str = "Content-Location";
/// Content-Language
pub const RESP_HEAD_CONTENT_LANGUAGE: &str = "Content-Language";
/// ETag
pub const RESP_HEAD_ETAG: &str = "ETag";
/// Sec-WebSocket-Accept
pub const RESP_HEAD_SEC_WEBSOCKET_ACCEPT: &str = "Sec-WebSocket-Accept";

#[allow(missing_docs)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResponseHeader<'a> {
    AccessControlAllowOrigin(&'a str),
    Connection(&'a str),
    Date(&'a str),
    KeepAlive(&'a str),
    LastModified(&'a str),
    Server(&'a str),
    SetCookie(&'a str),
    TransferEncoding(&'a str),
    Vary(&'a str),
    ContentLength(usize),
    ContentRange(&'a str),
    ContentType(&'a str),
    ContentEncoding(&'a str),
    ContentLocation(&'a str),
    ContentLanguage(&'a str),
    ETag(&'a str),
    SecWebSocketAccept([u8; 28]),
    Other(&'a str, &'a str),
}

impl<'a> HttpWrite for ResponseHeader<'a> {
    async fn write<T: Write>(self, writer: &mut T) -> Result<(), WriteError> {
        let len: AsciiInt;
        let ws_accept: [u8; 28];

        let val = match self {
            Self::AccessControlAllowOrigin(s) => {
                writer
                    .write_all(RESP_HEAD_ACCESS_CONTROL_ALLOW_ORIGIN.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::Connection(s) => {
                writer
                    .write_all(RESP_HEAD_CONNECTION.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::Date(s) => {
                writer
                    .write_all(RESP_HEAD_DATE.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::KeepAlive(s) => {
                writer
                    .write_all(RESP_HEAD_KEEP_ALIVE.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::LastModified(s) => {
                writer
                    .write_all(RESP_HEAD_LAST_MODIFIED.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::Server(s) => {
                writer
                    .write_all(RESP_HEAD_SERVER.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::SetCookie(s) => {
                writer
                    .write_all(RESP_HEAD_SET_COOKIE.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::TransferEncoding(s) => {
                writer
                    .write_all(RESP_HEAD_TRANSFER_ENCODING.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::Vary(s) => {
                writer
                    .write_all(RESP_HEAD_VARY.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::ContentLength(n) => {
                if n == 0 {
                    return Ok(());
                }
                writer
                    .write_all(RESP_HEAD_CONTENT_LENGTH.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;

                len = AsciiInt::from(n as u64);
                len.as_str()
            }
            Self::ContentRange(s) => {
                writer
                    .write_all(RESP_HEAD_CONTENT_RANGE.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::ContentType(s) => {
                writer
                    .write_all(RESP_HEAD_CONTENT_TYPE.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::ContentEncoding(s) => {
                writer
                    .write_all(RESP_HEAD_CONTENT_ENCODING.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::ContentLocation(s) => {
                writer
                    .write_all(RESP_HEAD_CONTENT_LOCATION.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::ContentLanguage(s) => {
                writer
                    .write_all(RESP_HEAD_CONTENT_LANGUAGE.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::ETag(s) => {
                writer
                    .write_all(RESP_HEAD_ETAG.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                s
            }
            Self::SecWebSocketAccept(s) => {
                writer
                    .write_all(RESP_HEAD_SEC_WEBSOCKET_ACCEPT.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                ws_accept = s;
                str::from_utf8(&ws_accept).unwrap()
            }
            Self::Other(k, v) => {
                writer
                    .write_all(k.as_bytes())
                    .await
                    .or(Err(WriteError::NetworkError))?;
                v
            }
        };

        writer
            .write_all(": ".as_bytes())
            .await
            .and(writer.write_all(val.as_bytes()).await)
            .and(writer.write_all(&[CR, LF]).await)
            .or(Err(WriteError::NetworkError))
    }
}
