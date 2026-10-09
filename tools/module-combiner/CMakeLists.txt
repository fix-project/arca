cmake_minimum_required(VERSION 3.18)
project(module-combiner LANGUAGES C CXX)

set(BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(BUILD_LIBWASM OFF CACHE BOOL "" FORCE)
set(WITH_WASI OFF CACHE BOOL "" FORCE)
add_subdirectory("${WABT_SOURCE}" wabt)
add_library(module-combiner STATIC
  combine-modules.cc generate-prefix-names.cc resolve-imports.cc)
target_compile_features(module-combiner PUBLIC cxx_std_20)
target_include_directories(module-combiner PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}" "${WABT_SOURCE}/include" "${CMAKE_BINARY_DIR}/wabt/include")
target_link_libraries(module-combiner PUBLIC wabt)
add_executable(wasm-merge wasmlink.cc)
target_link_libraries(wasm-merge PRIVATE module-combiner)
