#include "atha_p0_ffi.h"

#include <algorithm>
#include <array>
#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace {

class Library {
  public:
    explicit Library(const char *path) {
#if defined(_WIN32)
        handle_ = LoadLibraryA(path);
#else
        handle_ = dlopen(path, RTLD_NOW | RTLD_LOCAL);
#endif
        if (handle_ == nullptr) {
            throw std::runtime_error("could not load library: " + std::string(path));
        }
    }

    Library(const Library &) = delete;
    Library &operator=(const Library &) = delete;

    ~Library() {
#if defined(_WIN32)
        FreeLibrary(handle_);
#else
        dlclose(handle_);
#endif
    }

    template <typename Function>
    Function symbol(const char *name) const {
#if defined(_WIN32)
        auto address = GetProcAddress(handle_, name);
#else
        auto address = dlsym(handle_, name);
#endif
        if (address == nullptr) {
            throw std::runtime_error("missing ABI symbol: " + std::string(name));
        }
        return reinterpret_cast<Function>(address);
    }

  private:
#if defined(_WIN32)
    HMODULE handle_ = nullptr;
#else
    void *handle_ = nullptr;
#endif
};

struct Api {
    decltype(&atha_p0_abi_version) abi_version;
    decltype(&atha_p0_implementation) implementation;
    decltype(&atha_p0_noop) noop;
    decltype(&atha_p0_checksum) checksum;
    decltype(&atha_p0_string_clone) string_clone;
    decltype(&atha_p0_string_free) string_free;
};

template <typename Work>
std::pair<double, double> measure(size_t calls, size_t samples, Work work) {
    std::vector<double> elapsed;
    elapsed.reserve(samples);

    for (size_t sample = 0; sample < samples; ++sample) {
        const auto start = std::chrono::steady_clock::now();
        work(calls);
        const auto end = std::chrono::steady_clock::now();
        const auto nanoseconds =
            std::chrono::duration<double, std::nano>(end - start).count();
        elapsed.push_back(nanoseconds / static_cast<double>(calls));
    }

    std::sort(elapsed.begin(), elapsed.end());
    const size_t median_index = elapsed.size() / 2;
    const size_t p95_index = (elapsed.size() * 95 + 99) / 100 - 1;
    return {elapsed[median_index], elapsed[p95_index]};
}

void print_result(const char *implementation, const char *name, size_t calls,
                  size_t samples, const std::pair<double, double> &result) {
    std::cout << implementation << ',' << name << ',' << calls << ',' << samples
              << ',' << std::fixed << std::setprecision(2) << result.first << ','
              << result.second << '\n';
}

void verify(const Api &api) {
    if (api.abi_version() != ATHA_P0_ABI_VERSION) {
        throw std::runtime_error("ABI version mismatch");
    }
    if (api.noop(42) != 42) {
        throw std::runtime_error("noop result mismatch");
    }

    constexpr std::array<uint8_t, 5> hello{'h', 'e', 'l', 'l', 'o'};
    if (api.checksum(hello.data(), hello.size()) != UINT64_C(0xa430d84680aabd0b)) {
        throw std::runtime_error("checksum result mismatch");
    }
    if (api.checksum(nullptr, 1) != 0) {
        throw std::runtime_error("checksum accepted a null buffer");
    }

    char *copy = nullptr;
    if (api.string_clone("Atha", &copy) != ATHA_P0_OK || copy == nullptr ||
        std::strcmp(copy, "Atha") != 0) {
        throw std::runtime_error("string clone result mismatch");
    }
    api.string_free(copy);
    if (api.string_clone(nullptr, &copy) != ATHA_P0_INVALID_ARGUMENT) {
        throw std::runtime_error("string clone accepted a null input");
    }
}

size_t parse_samples(const char *value) {
    char *end = nullptr;
    const unsigned long parsed = std::strtoul(value, &end, 10);
    if (end == value || *end != '\0' || parsed < 5 || parsed > 101) {
        throw std::runtime_error("samples must be an integer from 5 to 101");
    }
    return static_cast<size_t>(parsed);
}

} // namespace

int main(int argc, char **argv) {
    try {
        if (argc < 2 || argc > 3) {
            std::cerr << "usage: atha_p0_ffi_runner <library> [samples]\n";
            return 2;
        }

        const size_t samples = argc == 3 ? parse_samples(argv[2]) : 31;
        const Library library(argv[1]);
        const Api api{
            library.symbol<decltype(&atha_p0_abi_version)>("atha_p0_abi_version"),
            library.symbol<decltype(&atha_p0_implementation)>("atha_p0_implementation"),
            library.symbol<decltype(&atha_p0_noop)>("atha_p0_noop"),
            library.symbol<decltype(&atha_p0_checksum)>("atha_p0_checksum"),
            library.symbol<decltype(&atha_p0_string_clone)>("atha_p0_string_clone"),
            library.symbol<decltype(&atha_p0_string_free)>("atha_p0_string_free")};

        verify(api);
        const char *implementation = api.implementation();
        if (implementation == nullptr || *implementation == '\0') {
            throw std::runtime_error("implementation name is empty");
        }

        std::cout << "implementation,case,calls,samples,median_ns_per_call,p95_ns_per_call\n";
        volatile uint64_t sink = 0;
        for (const size_t calls : {size_t{1}, size_t{100}, size_t{10'000}}) {
            const auto result = measure(calls, samples, [&](size_t count) {
                uint64_t value = sink;
                for (size_t index = 0; index < count; ++index) {
                    value = api.noop(value + index);
                }
                sink = value;
            });
            print_result(implementation, "noop", calls, samples, result);
        }

        const auto string_result = measure(1'000, samples, [&](size_t count) {
            for (size_t index = 0; index < count; ++index) {
                char *copy = nullptr;
                if (api.string_clone("Atha FFI ownership", &copy) != ATHA_P0_OK) {
                    throw std::runtime_error("string clone failed during benchmark");
                }
                sink ^= static_cast<uint8_t>(copy[0]);
                api.string_free(copy);
            }
        });
        print_result(implementation, "string_clone", 1'000, samples, string_result);

        std::vector<uint8_t> bytes(1024 * 1024, 0x5a);
        const auto bytes_result = measure(10, samples, [&](size_t count) {
            for (size_t index = 0; index < count; ++index) {
                sink ^= api.checksum(bytes.data(), bytes.size());
            }
        });
        print_result(implementation, "checksum_1mib", 10, samples, bytes_result);
        return sink == UINT64_MAX ? 3 : 0;
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
