cmake_minimum_required(VERSION 3.21)

project(
    neebles-tray-host
    VERSION 1.0.5
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(
    Qt6 6.5 REQUIRED COMPONENTS
    Core
    Gui
    Qml
    Quick
    Network
)

find_package(
    LayerShellQt 6.3.4 REQUIRED
)

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(
    neebles-tray-host
    main.cpp
    traysocketclient.cpp
    traysocketclient.h
    ../shared/bosseventclient.cpp
    ../shared/bosseventclient.h
    ../shared/bosscommandclient.cpp
    ../shared/bosscommandclient.h
)

set_source_files_properties(
    ../assets/branding/neebles-boss-tray-icon.png
    PROPERTIES
        QT_RESOURCE_ALIAS neebles-boss-tray-icon.png
)

qt_add_qml_module(
    neebles-tray-host
    URI NEEBLES.TrayHost
    VERSION 1.0
    QML_FILES
        Main.qml
    RESOURCES
        ../assets/branding/neebles-boss-tray-icon.png
)

target_link_libraries(
    neebles-tray-host
    PRIVATE
        Qt6::Core
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::Network
        LayerShellQt::Interface
)
