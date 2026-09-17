cmake_minimum_required(VERSION 3.21)

project(
    neebles-launcher-events
    VERSION 1.0.0
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include("${CMAKE_CURRENT_LIST_DIR}/../../cmake/NeeblesQt.cmake")

find_package(
    Qt6 ${NEEBLES_QT_VERSION_RANGE} REQUIRED COMPONENTS
    Core
    Qml
    Network
)

qt_standard_project_setup(
    REQUIRES ${NEEBLES_QT_MIN_VERSION}
)

qt_add_library(
    neebles-launcher-events
    SHARED
)

qt_add_qml_module(
    neebles-launcher-events

    URI
        NEEBLES.BossEvents

    VERSION
        1.0

    SOURCES
        launcherbossevents.h
        ../shared/bosseventclient.cpp
        ../shared/bosseventclient.h
)

target_link_libraries(
    neebles-launcher-events

    PRIVATE
        Qt6::Core
        Qt6::Qml
        Qt6::Network
)

include(GNUInstallDirs)

set(
    NEEBLES_BOSS_EVENTS_QML_DIR
    "${CMAKE_INSTALL_LIBDIR}/qt6/qml/NEEBLES/BossEvents"
)

install(
    TARGETS
        neebles-launcher-events
        neebles-launcher-eventsplugin

    LIBRARY DESTINATION
        "${NEEBLES_BOSS_EVENTS_QML_DIR}"
)

install(
    FILES
        "${CMAKE_CURRENT_BINARY_DIR}/qmldir"
        "${CMAKE_CURRENT_BINARY_DIR}/neebles-launcher-events.qmltypes"

    DESTINATION
        "${NEEBLES_BOSS_EVENTS_QML_DIR}"
)
