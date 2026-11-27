use std::{borrow::Cow, sync::RwLock};

use atha_backend::reader::{
    library::{LibraryError, LocalLibrary},
    resources::{Resource, ResourceError},
};
use tauri::http::{Request, Response, StatusCode, header};

#[cfg(any(windows, target_os = "android"))]
pub(crate) const TAURI_READER_ORIGIN: &str = "https://tauri.localhost";
#[cfg(not(any(windows, target_os = "android")))]
pub(crate) const TAURI_READER_ORIGIN: &str = "tauri://localhost";

pub(crate) fn book_response(
    current: &RwLock<crate::ReaderSession>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    let current = match current.read() {
        Ok(value) => value,
        Err(_) => {
            log::error!(
                target: "atha::protocol",
                "operation=book_resource outcome=failed code=reader-state"
            );
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let Some(root) = current.root.as_ref() else {
        return empty_response(StatusCode::NOT_FOUND);
    };
    match root.read(request.uri().path()) {
        Ok(resource) => resource_response(resource, "no-store", true),
        Err(error) => {
            if is_internal_resource_error(error) {
                log::error!(
                    target: "atha::protocol",
                    "operation=book_resource outcome=failed code={}",
                    error.code()
                );
            }
            empty_response(resource_status(error))
        }
    }
}

pub(crate) fn cover_response(
    library: &LocalLibrary,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    let id = request.uri().path().strip_prefix('/').unwrap_or_default();
    if id.contains('/') {
        return empty_response(StatusCode::BAD_REQUEST);
    }
    match library.cover(id) {
        Ok(resource) => resource_response(resource, "no-store", false),
        Err(LibraryError::InvalidBookId) => empty_response(StatusCode::BAD_REQUEST),
        Err(LibraryError::UnknownBook | LibraryError::MissingCover) => {
            empty_response(StatusCode::NOT_FOUND)
        }
        Err(LibraryError::Resource(error)) => {
            if is_internal_resource_error(error) {
                log::error!(
                    target: "atha::protocol",
                    "operation=cover_resource outcome=failed code={}",
                    error.code()
                );
            }
            empty_response(resource_status(error))
        }
        Err(error) => {
            log::error!(
                target: "atha::protocol",
                "operation=cover_resource outcome=failed code={}",
                error.code()
            );
            empty_response(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn resource_response(
    resource: Resource,
    cache_control: &'static str,
    cors: bool,
) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, resource.content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .header("x-content-type-options", "nosniff");
    if cors {
        response = response.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, TAURI_READER_ORIGIN);
    }
    response
        .body(Cow::Owned(resource.bytes))
        .expect("valid resource response")
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

const fn is_internal_resource_error(error: ResourceError) -> bool {
    matches!(
        error,
        ResourceError::InvalidRoot | ResourceError::ReadFailed
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_internal_resource_failures_are_log_worthy() {
        assert!(is_internal_resource_error(ResourceError::InvalidRoot));
        assert!(is_internal_resource_error(ResourceError::ReadFailed));
        assert!(!is_internal_resource_error(ResourceError::InvalidPath));
        assert!(!is_internal_resource_error(ResourceError::NotFound));
    }
}
