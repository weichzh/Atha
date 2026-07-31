#ifndef ATHA_P0_FFI_H
#define ATHA_P0_FFI_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32) && defined(ATHA_P0_FFI_EXPORTS)
#define ATHA_P0_API __declspec(dllexport)
#elif defined(__GNUC__) && defined(ATHA_P0_FFI_EXPORTS)
#define ATHA_P0_API __attribute__((visibility("default")))
#else
#define ATHA_P0_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define ATHA_P0_ABI_VERSION 1u

enum atha_p0_status {
    ATHA_P0_OK = 0,
    ATHA_P0_INVALID_ARGUMENT = 1,
    ATHA_P0_ALLOCATION_FAILED = 2
};

ATHA_P0_API uint32_t atha_p0_abi_version(void);
ATHA_P0_API const char *atha_p0_implementation(void);
ATHA_P0_API uint64_t atha_p0_noop(uint64_t value);
ATHA_P0_API uint64_t atha_p0_checksum(const uint8_t *data, size_t length);
ATHA_P0_API int32_t atha_p0_string_clone(const char *input, char **output);
ATHA_P0_API void atha_p0_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
