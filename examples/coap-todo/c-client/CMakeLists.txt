# todo-c: the C todo client, built on libcoap (CoAP) and exip (SenML EXI). Both
# libraries are built from the checkouts that ../scripts/fetch-third-party.sh makes.
#
#   cmake -S c-client -B build/c-client -DTODO_DEPS_DIR="$(scripts/fetch-third-party.sh --print-dir)"
#   cmake --build build/c-client
cmake_minimum_required(VERSION 3.16)
project(coap_todo_c_client C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(TODO_DEPS_DIR "" CACHE PATH "cache with exip/ and libcoap/ (scripts/fetch-third-party.sh --print-dir)")
if(NOT EXISTS "${TODO_DEPS_DIR}/exip/CMakeLists.txt" OR NOT EXISTS "${TODO_DEPS_DIR}/libcoap/CMakeLists.txt")
  message(FATAL_ERROR "run scripts/fetch-third-party.sh, then set TODO_DEPS_DIR to its --print-dir")
endif()

# Both libraries are built and installed into the build tree as external projects:
# exip's CMake files expect to be the top-level project.
include(ExternalProject)
set(DEPS_PREFIX ${CMAKE_BINARY_DIR}/deps)
file(MAKE_DIRECTORY ${DEPS_PREFIX}/include ${DEPS_PREFIX}/include/exip)

# libcoap: a plain UDP client, no DTLS, no extras.
ExternalProject_Add(libcoap_ext
  SOURCE_DIR ${TODO_DEPS_DIR}/libcoap
  CMAKE_ARGS -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=${DEPS_PREFIX}
             -DCMAKE_INSTALL_LIBDIR=lib -DBUILD_SHARED_LIBS=OFF
             -DENABLE_DTLS=OFF -DENABLE_OSCORE=OFF -DENABLE_TCP=OFF -DENABLE_WS=OFF
             -DENABLE_SERVER_MODE=OFF -DENABLE_PROXY_CODE=OFF -DENABLE_EXAMPLES=OFF
             -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF
  BUILD_BYPRODUCTS ${DEPS_PREFIX}/lib/libcoap-3.a)

# exip: the five static libraries and the headers, no tools, tests or zlib.
set(EXIP_LIBS contentIO grammarGen grammar streamIO common)
set(EXIP_BYPRODUCTS)
foreach(l ${EXIP_LIBS})
  list(APPEND EXIP_BYPRODUCTS ${DEPS_PREFIX}/lib/libexip.${l}.a)
endforeach()
ExternalProject_Add(exip_ext
  SOURCE_DIR ${TODO_DEPS_DIR}/exip
  CMAKE_ARGS -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=${DEPS_PREFIX}
             -DCMAKE_INSTALL_LIBDIR=lib -DEXIP_BUILD_TOOLS=OFF -DEXIP_BUILD_EXAMPLES=OFF
             -DEXIP_BUILD_TESTS=OFF -DEXIP_USE_ZLIB=OFF -DEXIP_WARNINGS_AS_ERRORS=OFF
  BUILD_BYPRODUCTS ${EXIP_BYPRODUCTS})

find_package(Threads REQUIRED)

add_executable(todo-c todo_client.c)
add_dependencies(todo-c libcoap_ext exip_ext)
# The SenML schema as exip loads it (scripts/gen-schemas.sh writes it).
target_compile_definitions(todo-c PRIVATE EXIP_HAVE_BUILD_CONFIG=1
  TODO_SENML_XSD_EXI="${CMAKE_CURRENT_SOURCE_DIR}/../schemas/senml.xsd.exi")
target_include_directories(todo-c PRIVATE ${DEPS_PREFIX}/include ${DEPS_PREFIX}/include/exip)
target_compile_options(todo-c PRIVATE -Wall -Wextra)
# The exip libraries call each other in both directions, so link them as a group.
target_link_libraries(todo-c PRIVATE
  ${DEPS_PREFIX}/lib/libcoap-3.a
  -Wl,--start-group ${EXIP_BYPRODUCTS} -Wl,--end-group
  Threads::Threads m)
