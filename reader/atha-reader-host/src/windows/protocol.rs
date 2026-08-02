use std::borrow::Cow;

use atha_backend::reader::{
    READER_ORIGIN,
    resources::{BookRoot, ResourceError},
};
use wry::http::{Request, Response, StatusCode, header};

use super::launch::APP_PAGE;

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src https://atha-book.localhost; connect-src 'self' https://atha-book.localhost; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";

pub(super) fn is_reader_url(url: String) -> bool {
    let Some(suffix) = url.strip_prefix(APP_PAGE) else {
        return false;
    };
    suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
}

pub(super) fn app_response(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    let (body, content_type): (Cow<'static, [u8]>, &'static str) = match request.uri().path() {
        "/atha-reader.html" => (
            Cow::Borrowed(include_bytes!("../../../atha-reader.html")),
            "text/html; charset=utf-8",
        ),
        "/atha-reader.css" => (
            Cow::Borrowed(include_bytes!("../../../atha-reader.css")),
            "text/css; charset=utf-8",
        ),
        "/atha-reader.mjs" => (
            Cow::Owned(
                [
                    include_bytes!("../../../web/content.mjs").as_slice(),
                    include_bytes!("../../../web/pagination.mjs").as_slice(),
                    include_bytes!("../../../web/diagnostics.mjs").as_slice(),
                    include_bytes!("../../../web/app.mjs").as_slice(),
                ]
                .concat(),
            ),
            "text/javascript; charset=utf-8",
        ),
        _ => return empty_response(StatusCode::NOT_FOUND),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_SECURITY_POLICY, CSP)
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .body(body)
        .expect("valid app response")
}

pub(super) fn book_response(
    root: &BookRoot,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    match root.read(request.uri().path()) {
        Ok(resource) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, resource.content_type)
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, READER_ORIGIN)
            .header("x-content-type-options", "nosniff")
            .body(Cow::Owned(resource.bytes))
            .expect("valid book response"),
        Err(error) => empty_response(resource_status(error)),
    }
}

fn empty_response(status: StatusCode) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Cow::Borrowed(&b"resource unavailable"[..]))
        .expect("valid error response")
}

const fn resource_status(error: ResourceError) -> StatusCode {
    match error {
        ResourceError::InvalidEncoding | ResourceError::InvalidPath => StatusCode::BAD_REQUEST,
        ResourceError::OutsideRoot => StatusCode::FORBIDDEN,
        ResourceError::NotFound | ResourceError::NotAFile => StatusCode::NOT_FOUND,
        ResourceError::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ResourceError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        ResourceError::InvalidRoot | ResourceError::ReadFailed => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
