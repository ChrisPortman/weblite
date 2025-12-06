# WebLite

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/badge/crates.io-v0.0.1-blue
[crates-url]: https://crates.io/crates/weblite
[docs-badge]: https://img.shields.io/badge/docs.rs-v0.0.1-blue
[docs-url]: https://docs.rs/weblite/0.0.1/weblite/
[mit-url]: https://github.com/ChrisPortman/weblite/blob/main/LICENSE.txt

`weblite` is a **very** basic implementation of the HTTP protocol predominantly aimed at
`no_std` and `no_alloc` use cases such as embedded development.

This crate provides:

* encoding and decoding of HTTP requests and responses on the "wire" respectively.
* encoding and decoding of websocket frames on the "wire".

This crate does **not** provide:

* any mechanism for routing requests to specific handlers.
* any higher level functionality for extracting data from paths, or request bodies.

## Example Pattern

My own usage pattern, for an embedded project has been to create a single HTML artefact (`index.html`)
inlines all required javascript (`<script>...</script>`) and CSS `<head><style>...</style></head>`)
as well as SVG images. The HTML is then embedded in the built binary as a `const &[u8]` with `include_bytes!("html/index.html");`.

The handler `weblite::server::RequestHandler` implementation then only handles:

* `/` - responds with the HTML
* `/ws` - starts a websocket
* *other* - responds with 404.

The client requests `/` and gets the single page back.  The javascript starts a websocket with
`/ws`.  All further interaction is done via the websocket, with the javascript manipulating the DOM
in response to events and commands received on the socket.

There's no reason you can't have a more complicated URL landscape, but this works for me.

## Examples

See the crate docs for example usage.
