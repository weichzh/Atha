cmake_minimum_required(VERSION 3.25)
project(AthaP0Ffi LANGUAGES CXX)

include(CTest)

add_library(atha_p0_cpp SHARED cpp/atha_p0_ffi.cpp)
target_compile_definitions(atha_p0_cpp PRIVATE ATHA_P0_FFI_EXPORTS)
target_compile_features(atha_p0_cpp PRIVATE cxx_std_17)
target_include_directories(atha_p0_cpp PUBLIC include)

add_executable(atha_p0_ffi_runner runner/main.cpp)
target_compile_features(atha_p0_ffi_runner PRIVATE cxx_std_17)
target_include_directories(atha_p0_ffi_runner PRIVATE include)

if(MSVC)
    target_compile_options(atha_p0_cpp PRIVATE /W4 /WX /permissive-)
    target_compile_options(atha_p0_ffi_runner PRIVATE /W4 /WX /permissive-)
else()
    target_compile_options(atha_p0_cpp PRIVATE -Wall -Wextra -Wpedantic -Werror)
    target_compile_options(atha_p0_ffi_runner PRIVATE -Wall -Wextra -Wpedantic -Werror)
    target_link_libraries(atha_p0_ffi_runner PRIVATE ${CMAKE_DL_LIBS})
endif()

if(BUILD_TESTING)
    add_test(
        NAME p0_ffi_cpp
        COMMAND atha_p0_ffi_runner $<TARGET_FILE:atha_p0_cpp> 11
    )
endif()
