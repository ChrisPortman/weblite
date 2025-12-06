use core::mem::discriminant;

use crate::ascii::{COLON, CR, LF, SP};
use crate::header::RequestHeader;

const GET: &[u8] = "GET".as_bytes();
const POST: &[u8] = "POST".as_bytes();
const PUT: &[u8] = "PUT".as_bytes();
const PATCH: &[u8] = "PATCH".as_bytes();
const DELETE: &[u8] = "DELETE".as_bytes();
const OPTIONS: &[u8] = "OPTIONS".as_bytes();
const HEAD: &[u8] = "HEAD".as_bytes();

#[derive(PartialEq, Debug)]
pub(crate) enum RequestError {
    Incomplete(Option<usize>),
    ProtocolError(&'static str),
}

/// Method such as GET. POST, DELETE etc.
#[non_exhaustive]
#[derive(PartialEq, Debug)]
pub enum Method {
    #[allow(missing_docs)]
    GET,
    #[allow(missing_docs)]
    POST,
    #[allow(missing_docs)]
    PUT,
    #[allow(missing_docs)]
    PATCH,
    #[allow(missing_docs)]
    DELETE,
    #[allow(missing_docs)]
    OPTIONS,
    #[allow(missing_docs)]
    HEAD,
}

impl TryFrom<&[u8]> for Method {
    type Error = &'static str;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value {
            GET => Ok(Self::GET),
            POST => Ok(Self::POST),
            PUT => Ok(Self::PUT),
            PATCH => Ok(Self::PATCH),
            DELETE => Ok(Self::DELETE),
            OPTIONS => Ok(Self::OPTIONS),
            HEAD => Ok(Self::HEAD),
            _ => Err("unknown http method"),
        }
    }
}

/// Request comtains the details of the request parsed from bytes read from the client
#[non_exhaustive]
#[derive(Debug)]
pub struct Request<'a> {
    /// Method (GET, POST etc) parsed from the request
    pub method: Method,
    /// URL path e.g. `/index.html?foo=bar`
    pub path: &'a str,
    /// Host extracted from the host header
    pub host: &'a str,
    /// Content-Type extracted from the Content-Type header where present
    pub content_type: Option<&'a str>,
    /// User agent extracted from the User-Agent header where present
    pub user_agent: Option<&'a str>,
    /// Content length extracted from the Content-Length header if present else 0
    pub content_length: usize,
    pub(crate) body: Option<&'a [u8]>,
    pub(crate) header_slice: Option<&'a [u8]>,
}

impl<'a> Request<'a> {
    /// Parse the provided byte slice returning a Request or an error.
    pub(crate) fn parse(data: &'a [u8]) -> Result<Self, RequestError> {
        // ensure upfront we have valid utf8 so later we can just unwrap str conversions
        if str::from_utf8(data).is_err() {
            return Err(RequestError::ProtocolError(
                "http request is not valid utf8",
            ));
        }

        let mut req = Request {
            method: Method::GET,
            path: "",
            host: "",
            content_type: None,
            user_agent: None,
            content_length: 0,
            header_slice: None,
            body: None,
        };

        let mut request_line_done = false;
        let mut http_headers_done = false;
        let mut header_start_offset = 0usize;
        let mut header_end_offset = 0usize;

        let mut line_start = 0;
        for i in 0..=data.len() {
            if let [CR, LF] = &data[line_start..i] {
                // a \r\n imediately after a line\r\n indicates the end of the headers
                http_headers_done = true;

                if req.content_length > 0 {
                    req.body = data.get(i..i + req.content_length);
                    if req.body.is_none() {
                        return Err(RequestError::Incomplete(Some(req.content_length)));
                    }
                }

                break;
            }

            if let [line @ .., CR, LF] = &data[line_start..i] {
                if !request_line_done {
                    req.parse_request_line(line)?;
                    request_line_done = true;
                } else {
                    req.parse_header_line(line)?;
                    if header_start_offset == 0 {
                        header_start_offset = line_start;
                    }
                    header_end_offset = i;
                }
                line_start = i;
            }
        }

        if header_start_offset != 0 && header_end_offset != 0 {
            req.header_slice = Some(&data[header_start_offset..header_end_offset])
        }

        if !http_headers_done {
            return Err(RequestError::Incomplete(None));
        }

        if req.path.is_empty() {
            return Err(RequestError::ProtocolError("malformed HTTP request"));
        }

        Ok(req)
    }

    fn parse_request_line(&mut self, data: &'a [u8]) -> Result<(), RequestError> {
        for (i, word) in data.splitn(3, |b: &u8| *b == SP).enumerate() {
            match i {
                0 => match Method::try_from(word) {
                    Ok(m) => self.method = m,
                    Err(_) => return Err(RequestError::ProtocolError("unknown http method")),
                },
                1 => self.path = str::from_utf8(word).unwrap(),
                2 => {}
                _ => return Err(RequestError::ProtocolError("malformed http request")),
            };
        }

        Ok(())
    }

    fn parse_header_line(&mut self, data: &'a [u8]) -> Result<(), RequestError> {
        let mut header: Option<&'a str> = None;
        let mut value: Option<&'a str> = None;

        for (i, word) in data.splitn(2, |b: &u8| *b == COLON).enumerate() {
            match i {
                0 => {
                    header = Some(str::from_utf8(word).unwrap().trim());
                }
                1 => {
                    value = Some(str::from_utf8(word).unwrap().trim());
                }
                _ => return Err(RequestError::ProtocolError("malformed http request")),
            }
        }

        if let Some(header) = header
            && let Some(value) = value
        {
            match RequestHeader::try_from((header, value)) {
                Ok(h) => {
                    if let RequestHeader::ContentLength(l) = h {
                        self.content_length = l;
                        return Ok(());
                    }
                    if let RequestHeader::Host(s) = h {
                        self.host = s;
                        return Ok(());
                    }
                    if let RequestHeader::ContentType(s) = h {
                        self.content_type = Some(s);
                        return Ok(());
                    }
                    if let RequestHeader::UserAgent(s) = h {
                        self.user_agent = Some(s);
                        return Ok(());
                    }

                    return Ok(());
                }
                Err(None) => {
                    return Ok(());
                }
                Err(Some(e)) => {
                    return Err(RequestError::ProtocolError(e));
                }
            }
        }

        Ok(())
    }

    fn resolve_header(&self, data: &'a [u8]) -> Result<Option<RequestHeader<'a>>, RequestError> {
        let mut header: Option<&'a str> = None;
        let mut value: Option<&'a str> = None;

        for (i, word) in data.splitn(2, |b: &u8| *b == COLON).enumerate() {
            match i {
                0 => {
                    header = Some(str::from_utf8(word).unwrap().trim());
                }
                1 => {
                    value = Some(str::from_utf8(word).unwrap().trim());
                }
                _ => return Err(RequestError::ProtocolError("malformed http request")),
            }
        }

        if let Some(header) = header
            && let Some(value) = value
        {
            match RequestHeader::try_from((header, value)) {
                Ok(h) => {
                    return Ok(Some(h));
                }
                Err(None) => {
                    return Ok(None);
                }
                Err(Some(e)) => {
                    return Err(RequestError::ProtocolError(e));
                }
            }
        }

        Ok(None)
    }

    /// Search the portion of the oringinal byte slice that contained headers for a header matching
    /// the provided variant.
    /// Note: A number of header values are extracted during the initial parse which should be used
    /// in favor of this method which requireds a scan of the original headers each call.
    pub fn get_header(&self, header: RequestHeader<'_>) -> Option<RequestHeader<'a>> {
        if let Some(data) = self.header_slice {
            let mut line_start = 0;

            for i in 0..=data.len() {
                if let [line @ .., CR, LF] = &data[line_start..i] {
                    if let Ok(Some(h)) = self.resolve_header(line) {
                        match (header, h) {
                            (RequestHeader::Other(key1, _), RequestHeader::Other(key2, _))
                                if key1.eq_ignore_ascii_case(key2) =>
                            {
                                return Some(h);
                            }
                            (RequestHeader::Other(_, _), RequestHeader::Other(_, _)) => {}
                            (h1, h2) if discriminant(&h1) == discriminant(&h2) => {
                                return Some(h);
                            }
                            _ => {}
                        };
                    }
                    line_start = i;
                }
            }
        };

        None
    }

    /// Returns a reference to the request body bytes if any.
    pub fn get_body(&self) -> Option<&'a [u8]> {
        self.body
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn test_http_request_parsing_single_receive() {
        let req = "GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n".as_bytes();

        let req = Request::parse(req).unwrap();
        assert!(req.method == Method::GET);
        assert!(req.path == "/");
        assert!(req.content_length == 0, "{:?}", req);

        let req = "GET /index.html HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc".as_bytes();

        let req = Request::parse(req).unwrap();
        assert!(req.method == Method::GET);
        assert!(req.path == "/index.html");
        assert!(req.content_length == 3, "{:?}", req);
        assert_eq!(req.get_body(), Some("abc".as_bytes()));

        let req = "GET /index.html HTTP/1.1\r\ncontent-type: application/json\r\ncontent-length: 3\r\naccept: application/json\r\nAccept-Encoding: gzip\r\n\r\nabc".as_bytes();

        let req = Request::parse(req).unwrap();
        assert!(req.method == Method::GET);
        assert!(req.path == "/index.html");
        assert!(req.content_length == 3, "{:?}", req);
        assert_eq!(req.content_type, Some("application/json"));
        assert_eq!(
            req.get_header(RequestHeader::ContentType("")),
            Some(RequestHeader::ContentType("application/json"))
        );
        assert_eq!(
            req.get_header(RequestHeader::AcceptEncoding("")),
            Some(RequestHeader::AcceptEncoding("gzip"))
        );
        assert_eq!(
            req.get_header(RequestHeader::Accept("")),
            Some(RequestHeader::Accept("application/json"))
        );
        assert_eq!(req.get_body(), Some("abc".as_bytes()));
    }

    #[test]
    fn test_http_request_parsing_multiple_updates() {
        let mut http_buf = [0u8; 1024];
        let req_part_one = "GET / HTTP/1.1\r\nContentType:".as_bytes();
        let req_part_two = "application/json\r\n\r\n".as_bytes();

        http_buf[..req_part_one.len()].copy_from_slice(&req_part_one);
        http_buf[req_part_one.len()..req_part_one.len() + req_part_two.len()]
            .copy_from_slice(&req_part_two);

        let req = Request::parse(&http_buf[..]).unwrap();
        assert!(req.method == Method::GET);
        assert!(req.path == "/");
    }
}
