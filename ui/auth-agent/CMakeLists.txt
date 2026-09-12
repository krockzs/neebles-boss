cmake_minimum_required(VERSION 3.21)

project(
    neebles_auth_agent
    VERSION 1.0.4
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

find_package(
    Qt6 REQUIRED
    COMPONENTS
        Core
        Widgets
)

find_package(
    PkgConfig REQUIRED
)

pkg_check_modules(
    POLKITQT_AGENT
    REQUIRED
    IMPORTED_TARGET
    polkit-qt6-agent-1
)

pkg_check_modules(
    POLKITQT_CORE
    REQUIRED
    IMPORTED_TARGET
    polkit-qt6-core-1
)

qt_add_executable(
    neebles-auth-agent

    main.cpp
    authagent.cpp
    authagent.h
)

set(
    NEEBLES_LANG_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/../../client/languages"
)

qt_add_resources(
    neebles-auth-agent
    "neebles_auth_languages"

    PREFIX
        "/languages"

    BASE
        "${NEEBLES_LANG_DIR}"

    FILES
        "${NEEBLES_LANG_DIR}/en_US.json"
        "${NEEBLES_LANG_DIR}/es_CL.json"
)

target_link_libraries(
    neebles-auth-agent
    PRIVATE
        Qt6::Core
        Qt6::Widgets
        PkgConfig::POLKITQT_AGENT
        PkgConfig::POLKITQT_CORE
)

install(
    TARGETS neebles-auth-agent
    RUNTIME DESTINATION bin
)
