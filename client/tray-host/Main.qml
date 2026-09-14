import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    width: 520
    height: shell.implicitHeight

    visible: true

    title: root.t("tray.title")

    flags:
        Qt.FramelessWindowHint

    color: "transparent"

    property string selectedTrayId: ""

    function t(key) {
        if (
            typeof bossStrings !== "undefined"
            && bossStrings[key] !== undefined
        )
            return bossStrings[key]

        return key
    }

    readonly property int outerMargin: 12
    readonly property int headerHeight: 44
    readonly property int rowHeight: 68
    readonly property int rowSpacing: 7
    readonly property int actionWidth: 150

    Rectangle {
        id: shell

        anchors.fill: parent

        radius: 14

        color: "#121018"

        border.width: 1
        border.color: "#71449a"

        implicitHeight:
            outerMargin
            + headerHeight
            + 10
            + Math.max(
                trayList.contentHeight,
                selectedTrayId.length > 0
                    ? 104
                    : 0
            )
            + outerMargin
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: outerMargin

        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: headerHeight

            Label {
                text: root.t("tray.title")

                color: "#d7b8ff"

                font.pixelSize: 21
                font.bold: true

                Layout.fillWidth: true
            }

            Rectangle {
                width: 9
                height: 9
                radius: 5

                color:
                    trayClient.connected
                    ? "#53d9e8"
                    : "#666666"
            }
        }

        Label {
            visible:
                trayClient.error.length > 0

            text:
                trayClient.error

            color: "#ff7b9c"

            wrapMode:
                Text.Wrap

            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true

            spacing: 10

            ListView {
                id: trayList

                Layout.fillWidth: true

                Layout.preferredHeight:
                    contentHeight

                interactive: false

                spacing: rowSpacing

                model:
                    trayClient.model

                delegate: Rectangle {
                    id: trayRow

                    required property string trayId
                    required property string ownerModule
                    required property string moduleVersion
                    required property string icon
                    required property bool trayVisible
                    required property bool opened

                    width:
                        trayList.width

                    height:
                        trayVisible
                        ? rowHeight
                        : 0

                    visible:
                        trayVisible

                    radius: 8

                    readonly property bool selected:
                        root.selectedTrayId === trayId

                    color:
                        selected
                        ? "#271b35"
                        : "#19151f"

                    border.width: 1

                    border.color:
                        selected
                        ? "#9a58d0"
                        : "#3f334b"

                    MouseArea {
                        anchors.fill: parent

                        onClicked: {
                            root.selectedTrayId =
                                root.selectedTrayId === trayId
                                ? ""
                                : trayId
                        }
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 9

                        spacing: 11

                        Image {
                            source:
                                icon.length > 0
                                ? "file://" + icon
                                : ""

                            width: 34
                            height: 34

                            sourceSize.width: 34
                            sourceSize.height: 34

                            fillMode:
                                Image.PreserveAspectFit
                        }

                        ColumnLayout {
                            Layout.fillWidth: true

                            spacing: 2

                            Label {
                                text:
                                    ownerModule

                                color: "#5de4ef"

                                font.pixelSize: 15
                                font.bold: true
                            }

                            Label {
                                text:
                                    moduleVersion.length > 0
                                    ? "v" + moduleVersion
                                    : ""

                                color: "#c49be8"

                                font.pixelSize: 12
                            }
                        }

                        Rectangle {
                            width: 8
                            height: 8
                            radius: 4

                            color:
                                opened
                                ? "#9a58d0"
                                : "#55505a"
                        }
                    }
                }
            }

            Rectangle {
                id: actions

                visible:
                    root.selectedTrayId.length > 0

                Layout.preferredWidth:
                    visible
                    ? actionWidth
                    : 0

                Layout.preferredHeight: 104

                radius: 8

                color: "#17121d"

                border.width: 1
                border.color: "#9a58d0"

                property bool selectedOpened: false

                function refreshSelected() {
                    if (
                        root.selectedTrayId.length
                        === 0
                    ) {
                        selectedOpened = false
                        return
                    }

                    if (
                        !trayClient.model.containsTray(
                            root.selectedTrayId
                        )
                    ) {
                        root.selectedTrayId = ""
                        selectedOpened = false
                        return
                    }

                    selectedOpened =
                        trayClient.model.openedForTray(
                            root.selectedTrayId
                        )
                }

                Connections {
                    target:
                        trayClient.model

                    function onDataChanged() {
                        actions.refreshSelected()
                    }

                    function onModelReset() {
                        actions.refreshSelected()
                    }

                    function onRowsInserted() {
                        actions.refreshSelected()
                    }

                    function onRowsRemoved() {
                        actions.refreshSelected()
                    }
                }

                onVisibleChanged:
                    refreshSelected()

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12

                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            text: root.t("common.open")

                            color: "#e1d7e9"

                            Layout.fillWidth: true
                        }

                        Rectangle {
                            id: openSwitch

                            width: 38
                            height: 20
                            radius: 10

                            color:
                                actions.selectedOpened
                                ? "#9a58d0"
                                : "#4b4650"

                            border.width: 1

                            border.color:
                                actions.selectedOpened
                                ? "#c28cff"
                                : "#68616d"

                            Rectangle {
                                width: 16
                                height: 16
                                radius: 8

                                anchors.verticalCenter:
                                    parent.verticalCenter

                                x:
                                    actions.selectedOpened
                                    ? parent.width
                                        - width
                                        - 2
                                    : 2

                                color: "#f1eaf5"

                                Behavior on x {
                                    NumberAnimation {
                                        duration: 120
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent

                                cursorShape:
                                    Qt.PointingHandCursor

                                onClicked: {
                                    if (
                                        root.selectedTrayId.length
                                        === 0
                                    ) {
                                        return
                                    }

                                    if (
                                        actions.selectedOpened
                                    ) {
                                        trayClient.closeTray(
                                            root.selectedTrayId
                                        )
                                    } else {
                                        trayClient.openTray(
                                            root.selectedTrayId
                                        )
                                    }
                                }
                            }
                        }
                    }

                    Button {
                        text: root.t("tray.activate")

                        Layout.fillWidth: true

                        enabled:
                            root.selectedTrayId.length
                            > 0

                        onClicked:
                            trayClient.focusTray(
                                root.selectedTrayId
                            )
                    }
                }
            }
        }
    }
}
