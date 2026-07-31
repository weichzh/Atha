#include "atha_p0_ffi.h"

#include <cstring>
#include <new>

namespace {
constexpr uint64_t kFnvOffset = UINT64_C(14695981039346656037);
constexpr uint64_t kFnvPrime = UINT64_C(1099511628211);
}

uint32_t atha_p0_abi_version(void) {
    return ATHA_P0_ABI_VERSION;
}

const char *atha_p0_implementation(void) {
    return "cpp";
}

uint64_t atha_p0_noop(uint64_t value) {
    return value;
}

uint64_t atha_p0_checksum(const uint8_t *data, size_t length) {
    if (data == nullptr && length != 0) {
        return 0;
    }

    uint64_t hash = kFnvOffset;
    for (size_t index = 0; index < length; ++index) {
        hash ^= data[index];
        hash *= kFnvPrime;
    }
    return hash;
}

int32_t atha_p0_string_clone(const char *input, char **output) {
    if (input == nullptr || output == nullptr) {
        return ATHA_P0_INVALID_ARGUMENT;
    }

    *output = nullptr;
    const size_t length = std::strlen(input) + 1;
    auto *copy = new (std::nothrow) char[length];
    if (copy == nullptr) {
        return ATHA_P0_ALLOCATION_FAILED;
    }

    std::memcpy(copy, input, length);
    *output = copy;
    return ATHA_P0_OK;
}

void atha_p0_string_free(char *value) {
    delete[] value;
}
