cmake_minimum_required(VERSION 3.21)
project(neebles_tray VERSION 1.0.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 REQUIRED COMPONENTS Core DBus Gui Widgets)

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(neebles-tray
    main.cpp
)

target_link_libraries(neebles-tray
    PRIVATE
        Qt6::Core
        Qt6::DBus
        Qt6::Gui
        Qt6::Widgets
)

install(TARGETS neebles-tray
    RUNTIME DESTINATION bin
)
