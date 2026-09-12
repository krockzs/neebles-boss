cmake_minimum_required(VERSION 3.21)
project(neebles_ui VERSION 1.0.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Core DBus Quick QuickControls2)

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(neebles-ui
    main.cpp
    bosscontroller.cpp
    bosscontroller.h
)

qt_add_qml_module(neebles-ui
    URI NeeblesUI
    VERSION 1.0
    QML_FILES
        qml/Main.qml
)

target_link_libraries(neebles-ui
    PRIVATE
        Qt6::Core
        Qt6::DBus
        Qt6::Quick
        Qt6::QuickControls2
)

install(TARGETS neebles-ui
    RUNTIME DESTINATION bin
)
