cmake_minimum_required(VERSION 3.21)
project(neebles_installer VERSION 1.0.17 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include("${CMAKE_CURRENT_LIST_DIR}/../../cmake/NeeblesQt.cmake")

find_package(Qt6 ${NEEBLES_QT_VERSION} EXACT REQUIRED COMPONENTS Core Quick)

qt_standard_project_setup(REQUIRES ${NEEBLES_QT_VERSION})

qt_add_executable(neebles-installer
    main.cpp
    installercontroller.cpp
    installercontroller.h
    ../../client/shared/runtimeauthority.cpp
    ../../client/shared/runtimeauthority.h
    ../../client/shared/domesticprocess.h
)

# NEEBLES_QT_CONTROLLED_QMLCACHEGEN
#
# A Qt build tool taken from a controlled sysroot must execute
# under the loader and library authority of that same sysroot.
#
# This is intentionally scoped to qmlcachegen.  Native release
# builds without NEEBLES_QT_SYSROOT continue using Qt normally.
if(
    DEFINED NEEBLES_QT_SYSROOT
    AND NOT NEEBLES_QT_SYSROOT STREQUAL ""
)
    set(
        NEEBLES_QT_LIBRARY_DIR
        "${NEEBLES_QT_SYSROOT}/usr/lib/x86_64-linux-gnu"
    )

    set(
        NEEBLES_QT_LOADER
        "${NEEBLES_QT_LIBRARY_DIR}/ld-linux-x86-64.so.2"
    )

    set(
        NEEBLES_QMLCACHEGEN_REAL
        "${NEEBLES_QT_SYSROOT}/usr/lib/qt6/libexec/qmlcachegen"
    )

    foreach(
        NEEBLES_QT_REQUIRED_PATH
        IN ITEMS
            "${NEEBLES_QT_LOADER}"
            "${NEEBLES_QMLCACHEGEN_REAL}"
    )
        if(NOT EXISTS "${NEEBLES_QT_REQUIRED_PATH}")
            message(
                FATAL_ERROR
                "controlled Qt tool authority is incomplete: ${NEEBLES_QT_REQUIRED_PATH}"
            )
        endif()
    endforeach()

    set(
        NEEBLES_QMLCACHEGEN_WRAPPER
        "${CMAKE_CURRENT_BINARY_DIR}/neebles-qmlcachegen"
    )

    file(
        WRITE
        "${NEEBLES_QMLCACHEGEN_WRAPPER}"
        "#!/bin/sh\n"
        "exec \"${NEEBLES_QT_LOADER}\" "
        "--library-path \"${NEEBLES_QT_LIBRARY_DIR}\" "
        "\"${NEEBLES_QMLCACHEGEN_REAL}\" "
        "\"$@\"\n"
    )

    file(
        CHMOD
        "${NEEBLES_QMLCACHEGEN_WRAPPER}"
        PERMISSIONS
            OWNER_READ
            OWNER_WRITE
            OWNER_EXECUTE
            GROUP_READ
            GROUP_EXECUTE
            WORLD_READ
            WORLD_EXECUTE
    )

    list(
        PREPEND
        CMAKE_PROGRAM_PATH
        "${CMAKE_CURRENT_BINARY_DIR}"
    )

    set_target_properties(
        neebles-installer
        PROPERTIES
            QT_QMLCACHEGEN_EXECUTABLE
                "neebles-qmlcachegen"
    )

    # NEEBLES_QT_CONTROLLED_QMLIMPORTSCANNER
    #
    # qmlimportscanner belongs to the same controlled Qt world as
    # qmlcachegen.  Its imported target must therefore execute under
    # that world's loader and libraries instead of borrowing the host.

    set(
        NEEBLES_QMLIMPORTSCANNER_REAL
        "${NEEBLES_QT_SYSROOT}/usr/lib/qt6/libexec/qmlimportscanner"
    )

    if(NOT EXISTS "${NEEBLES_QMLIMPORTSCANNER_REAL}")
        message(
            FATAL_ERROR
            "controlled Qt qmlimportscanner is missing: ${NEEBLES_QMLIMPORTSCANNER_REAL}"
        )
    endif()

    set(
        NEEBLES_QMLIMPORTSCANNER_WRAPPER
        "${CMAKE_CURRENT_BINARY_DIR}/neebles-qmlimportscanner"
    )

    file(
        WRITE
        "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"
        "#!/bin/sh\n"
        "exec \"${NEEBLES_QT_LOADER}\" "
        "--library-path \"${NEEBLES_QT_LIBRARY_DIR}\" "
        "\"${NEEBLES_QMLIMPORTSCANNER_REAL}\" "
        "\"$@\"\n"
    )

    file(
        CHMOD
        "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"
        PERMISSIONS
            OWNER_READ
            OWNER_WRITE
            OWNER_EXECUTE
            GROUP_READ
            GROUP_EXECUTE
            WORLD_READ
            WORLD_EXECUTE
    )

    if(NOT TARGET Qt6::qmlimportscanner)
        message(
            FATAL_ERROR
            "Qt6::qmlimportscanner imported target is missing"
        )
    endif()

    set_target_properties(
        Qt6::qmlimportscanner
        PROPERTIES
            IMPORTED_LOCATION
                "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"

            IMPORTED_LOCATION_NONE
                "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"

            IMPORTED_LOCATION_DEBUG
                "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"

            IMPORTED_LOCATION_RELEASE
                "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"

            IMPORTED_LOCATION_RELWITHDEBINFO
                "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"

            IMPORTED_LOCATION_MINSIZEREL
                "${NEEBLES_QMLIMPORTSCANNER_WRAPPER}"
    )
endif()

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
