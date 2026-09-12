cmake_minimum_required(VERSION 3.21)
project(neebles_installer VERSION 1.0.4 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 REQUIRED COMPONENTS Core Quick)

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(neebles-installer
    main.cpp
    installercontroller.cpp
    installercontroller.h
)

qt_add_qml_module(neebles-installer
    URI NeeblesInstaller
    VERSION 1.0
    QML_FILES
        qml/Main.qml
)

target_link_libraries(neebles-installer
    PRIVATE
        Qt6::Core
        Qt6::Quick
)

install(TARGETS neebles-installer
    RUNTIME DESTINATION bin
)
