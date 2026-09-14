cmake_minimum_required(VERSION 3.21)

project(
    neebles-tray-host
    VERSION 1.0.5
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(
    Qt6 REQUIRED COMPONENTS
    Core
    Gui
    Qml
    Quick
    Network
)

find_package(
    LayerShellQt REQUIRED
)

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(
    neebles-tray-host
    main.cpp
    traysocketclient.cpp
    traysocketclient.h
)

qt_add_qml_module(
    neebles-tray-host
    URI NEEBLES.TrayHost
    VERSION 1.0
    QML_FILES
        Main.qml
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
