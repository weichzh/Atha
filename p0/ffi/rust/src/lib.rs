use std::ffi::{CStr, CString, c_char};
use std::ptr;

const ABI_VERSION: u32 = 1;
const OK: i32 = 0;
const INVALID_ARGUMENT: i32 = 1;
const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

#[unsafe(no_mangle)]
pub extern "C" fn atha_p0_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn atha_p0_implementation() -> *const c_char {
    c"rust".as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn atha_p0_noop(value: u64) -> u64 {
    value
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atha_p0_checksum(data: *const u8, length: usize) -> u64 {
    if data.is_null() && length != 0 {
        return 0;
    }

    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: The C ABI requires a readable buffer of exactly `length` bytes.
        unsafe { std::slice::from_raw_parts(data, length) }
    };

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atha_p0_string_clone(
    input: *const c_char,
    output: *mut *mut c_char,
) -> i32 {
    if input.is_null() || output.is_null() {
        return INVALID_ARGUMENT;
    }

    // SAFETY: Both pointers were checked above; callers own the input C string and output slot.
    unsafe { *output = ptr::null_mut() };
    // SAFETY: The C ABI requires `input` to be a valid NUL-terminated string.
    let input = unsafe { CStr::from_ptr(input) };
    let copy = CString::new(input.to_bytes()).expect("CStr cannot contain an interior NUL");
    // SAFETY: `output` is valid and the paired free function reclaims this allocation.
    unsafe { *output = copy.into_raw() };
    OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atha_p0_string_free(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: The C ABI only accepts pointers returned by `atha_p0_string_clone`.
        drop(unsafe { CString::from_raw(value) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_matches_fnv1a_reference() {
        let value = b"hello";
        let actual = unsafe { atha_p0_checksum(value.as_ptr(), value.len()) };
        assert_eq!(actual, 0xa430_d846_80aa_bd0b);
    }

    #[test]
    fn string_round_trip_uses_paired_free() {
        let input = c"Atha";
        let mut output = ptr::null_mut();

        let status = unsafe { atha_p0_string_clone(input.as_ptr(), &mut output) };

        assert_eq!(status, OK);
        assert_eq!(unsafe { CStr::from_ptr(output) }, input);
        unsafe { atha_p0_string_free(output) };
    }
}
