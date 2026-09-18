cmake_minimum_required(VERSION 3.21)
project(neebles_installer VERSION 1.0.12 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include("${CMAKE_CURRENT_LIST_DIR}/../../cmake/NeeblesQt.cmake")

find_package(Qt6 ${NEEBLES_QT_VERSION_RANGE} REQUIRED COMPONENTS Core Quick)

qt_standard_project_setup(REQUIRES ${NEEBLES_QT_MIN_VERSION})

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

qt_add_resources(neebles-installer "neebles_installer_assets"
    PREFIX "/neebles/installer"
    BASE "../../client/assets/branding"
    FILES
        ../../client/assets/branding/neebles-installer-icon.png
)

target_link_libraries(neebles-installer
    PRIVATE
        Qt6::Core
        Qt6::Quick
)

install(TARGETS neebles-installer
    RUNTIME DESTINATION bin
)
