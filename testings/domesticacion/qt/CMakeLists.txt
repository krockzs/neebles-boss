cmake_minimum_required(VERSION 3.21)

project(
    neebles_domestic_qt_cert
    LANGUAGES CXX
)

set(
    CMAKE_CXX_STANDARD
    17
)

set(
    CMAKE_CXX_STANDARD_REQUIRED
    ON
)

set(
    CMAKE_SKIP_BUILD_RPATH
    TRUE
)

set(
    CMAKE_SKIP_INSTALL_RPATH
    TRUE
)

if(NOT DEFINED NEEBLES_QT_SYSROOT)
    message(
        FATAL_ERROR
        "NEEBLES_QT_SYSROOT is required"
    )
endif()

set(
    NEEBLES_QT_PREFIX
    "${NEEBLES_QT_SYSROOT}/usr"
)

set(
    NEEBLES_QT_LIBRARY_DIR
    "${NEEBLES_QT_PREFIX}/lib/${CMAKE_LIBRARY_ARCHITECTURE}"
)

set(
    Qt6_DIR
    "${NEEBLES_QT_LIBRARY_DIR}/cmake/Qt6"
    CACHE PATH
    "Controlled N.E.E.B.L.E.S. Qt directory"
    FORCE
)

list(
    PREPEND
    CMAKE_PREFIX_PATH
    "${NEEBLES_QT_PREFIX}"
)

include(
    "${CMAKE_CURRENT_LIST_DIR}/../../../cmake/NeeblesQt.cmake"
)

find_package(
    Qt6
    ${NEEBLES_QT_VERSION}
    EXACT
    REQUIRED
    COMPONENTS
        Core
)

qt_add_executable(
    neebles-domestic-qt-cert
    main.cpp
)

target_link_libraries(
    neebles-domestic-qt-cert
    PRIVATE
        Qt6::Core
)

target_link_options(
    neebles-domestic-qt-cert
    PRIVATE
        "LINKER:-rpath-link,${NEEBLES_QT_LIBRARY_DIR}"
)

set(
    NEEBLES_DOMESTIC_TEST_ROOT
    "${CMAKE_CURRENT_LIST_DIR}/.."
)

enable_testing()

add_test(
    NAME
        neebles-domestic-qt-matrix
    COMMAND
        "${NEEBLES_QT_LIBRARY_DIR}/ld-linux-x86-64.so.2"
        --library-path
        "${NEEBLES_QT_LIBRARY_DIR}"
        "$<TARGET_FILE:neebles-domestic-qt-cert>"
        "${NEEBLES_DOMESTIC_TEST_ROOT}/categories/environment/tester.json"
        "${NEEBLES_DOMESTIC_TEST_ROOT}/categories/allowed_session_inputs/tester.json"
)

# NEEBLES_RUNTIME_AUTHORITY_QT_CERT
qt_add_executable(
    neebles-runtime-authority-qt-cert

    runtimeauthority_main.cpp
    ../../../client/shared/runtimeauthority.cpp
    ../../../client/shared/runtimeauthority.h
    ../../../client/shared/domesticprocess.h
)

target_include_directories(
    neebles-runtime-authority-qt-cert

    PRIVATE
        "${CMAKE_CURRENT_LIST_DIR}/../../../client/shared"
)

target_link_libraries(
    neebles-runtime-authority-qt-cert

    PRIVATE
        Qt6::Core
)

target_link_options(
    neebles-runtime-authority-qt-cert

    PRIVATE
        "LINKER:-rpath-link,${NEEBLES_QT_LIBRARY_DIR}"
)

set_target_properties(
    neebles-runtime-authority-qt-cert

    PROPERTIES
        BUILD_RPATH
            "${NEEBLES_QT_LIBRARY_DIR}"
)
