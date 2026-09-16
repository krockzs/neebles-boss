import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    readonly property int collapsedWidth: 210
    readonly property int expandedWidth: 390

    width: expandedWidth

    height: shell.implicitHeight

    visible: false
    title: root.t("tray.title")

    flags: Qt.FramelessWindowHint
    color: "transparent"

    property string selectedTrayId: ""
    property var activeByTray: ({})

    function trayActive(trayId) {
        if (trayId.length === 0)
            return false

        return activeByTray[trayId] === true
    }

    function setTrayActive(trayId, active) {
        if (trayId.length === 0)
            return

        var next = Object.assign({}, activeByTray)
        next[trayId] = active
        activeByTray = next
    }

    function t(key) {
        if (
            typeof bossStrings !== "undefined"
            && bossStrings[key] !== undefined
        )
            return bossStrings[key]

        return key
    }


    component NeeblesButton: Button {
        id: control

        implicitHeight: 26

        font.pixelSize: 11
        font.bold: true

        contentItem: Text {
            text: control.text
            font: control.font

            color:
                control.enabled
                ? "#ffffff"
                : "#8c8c8c"

            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        background: Rectangle {
            radius: 6

            color:
                !control.enabled
                ? "#000000"
                : control.down
                    ? "#4d1670"
                    : control.hovered
                        ? "#6f2699"
                        : "#000000"

            border.width: 1

            border.color:
                !control.enabled
                ? "#50345f"
                : control.hovered || control.down
                    ? "#c26cff"
                    : "#8746ad"

            opacity:
                control.enabled
                ? 1.0
                : 0.55
        }
    }

    readonly property int outerMargin: 8
    readonly property int headerHeight: 24

    readonly property int fixedWidth: 178
    readonly property int actionsWidth: 170

    readonly property int moduleRowHeight: 40
    readonly property int moduleSpacing: 4
    readonly property int moduleMaximumRows: 3

    readonly property int moduleMaximumHeight:
        moduleRowHeight * moduleMaximumRows
        + moduleSpacing * (moduleMaximumRows - 1)

    readonly property int moduleListHeight:
        Math.max(
            moduleRowHeight,
            Math.min(
                trayList.contentHeight,
                moduleMaximumHeight
            )
        )

    Rectangle {
        id: shell

        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.right: parent.right

        width:
            root.selectedTrayId.length > 0
            ? root.expandedWidth
            : root.collapsedWidth

        radius: 10
        color: "#000000"

        border.width: 1
        border.color: "#71449a"

        implicitHeight:
            outerMargin
            + headerHeight
            + 5
            + Math.max(
                fixedColumn.implicitHeight,
                moduleActions.visible
                ? moduleActions.implicitHeight
                : 0
            )
            + outerMargin
    }

    ColumnLayout {
        anchors.fill: shell
        anchors.margins: outerMargin

        spacing: 5

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: headerHeight

            Image {
                id: bossIcon

                source: "qrc:/qt/qml/NEEBLES/TrayHost/neebles-boss-tray-icon.png"

                width: 20
                height: 20

                sourceSize.width: 20
                sourceSize.height: 20

                fillMode: Image.PreserveAspectFit
            }

            Item {
                Layout.fillWidth: true
            }

            Rectangle {
                width: 6
                height: 6
                radius: 3

                color:
                    trayClient.connected
                    ? "#00e5ff"
                    : "#6d6d6d"

                border.width: 1

                border.color:
                    trayClient.connected
                    ? "#ffffff"
                    : "#6d6d6d"
            }
        }

        Label {
            visible:
                trayClient.error.length > 0

            text:
                trayClient.error

            color: "#ff7b9c"

            font.pixelSize: 10

            wrapMode: Text.Wrap

            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true

            spacing: 7

            /*
             * VARIABLE LEFT SIDE.
             *
             * Because the whole LayerShell window is anchored
             * to the right edge, this area expands to the left.
             */
            Rectangle {
                id: moduleActions

                implicitHeight:
                    actionsContent.implicitHeight + 16

                visible:
                    root.selectedTrayId.length > 0

                Layout.preferredWidth:
                    visible
                    ? actionsWidth
                    : 0

                Layout.minimumWidth:
                    visible
                    ? actionsWidth
                    : 0

                Layout.maximumWidth:
                    visible
                    ? actionsWidth
                    : 0

                Layout.fillHeight: true

                radius: 7

                color: "#000000"

                border.width: 1
                border.color: "#71449a"

                property bool selectedActive:
                    root.trayActive(root.selectedTrayId)

                property bool selectedOpened: false

                function refreshSelected() {
                    if (
                        root.selectedTrayId.length === 0
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
                    target: trayClient.model

                    function onDataChanged() {
                        moduleActions.refreshSelected()
                    }

                    function onModelReset() {
                        moduleActions.refreshSelected()
                    }

                    function onRowsInserted() {
                        moduleActions.refreshSelected()
                    }

                    function onRowsRemoved() {
                        moduleActions.refreshSelected()
                    }
                }

                Connections {
                    target: root

                    function onSelectedTrayIdChanged() {
                        moduleActions.refreshSelected()
                    }
                }

                onVisibleChanged:
                    refreshSelected()

                ColumnLayout {
                    id: actionsContent

                    anchors.fill: parent
                    anchors.margins: 8

                    spacing: 7

                    Label {
                        text:
                            root.selectedTrayId

                        color: "#d7b8ff"

                        font.pixelSize: 12
                        font.bold: true

                        elide: Text.ElideRight

                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            text:
                                root.t("tray.activate")

                            color: "#ffffff"

                            font.pixelSize: 11

                            Layout.fillWidth: true
                        }

                        Rectangle {
                            width: 34
                            height: 18
                            radius: 9

                            color:
                                moduleActions.selectedActive
                                ? "#8d3ac7"
                                : "#000000"

                            border.width: 1

                            border.color:
                                moduleActions.selectedActive
                                ? "#c26cff"
                                : "#8746ad"

                            Rectangle {
                                width: 14
                                height: 14
                                radius: 7

                                anchors.verticalCenter:
                                    parent.verticalCenter

                                x:
                                    moduleActions.selectedActive
                                    ? parent.width - width - 2
                                    : 2

                                color: "#ffffff"

                                Behavior on x {
                                    NumberAnimation {
                                        duration: 100
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent

                                cursorShape:
                                    Qt.PointingHandCursor

                                onClicked: {
                                    if (
                                        root.selectedTrayId.length === 0
                                    ) {
                                        return
                                    }

                                    const trayId =
                                        root.selectedTrayId

                                    if (moduleActions.selectedActive) {
                                        root.setTrayActive(
                                            trayId,
                                            false
                                        )

                                        if (
                                            moduleActions.selectedOpened
                                        ) {
                                            trayClient.closeTray(
                                                trayId
                                            )
                                        }

                                        moduleActions.selectedOpened = false
                                    } else {
                                        root.setTrayActive(
                                            trayId,
                                            true
                                        )
                                    }
                                }
                            }
                        }
                    }

                    NeeblesButton {
                        visible:
                            moduleActions.selectedActive

                        enabled:
                            !moduleActions.selectedOpened

                        text:
                            root.t("common.open")

                        Layout.fillWidth: true

                        onClicked: {
                            trayClient.openTray(
                                root.selectedTrayId
                            )
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                    }
                }
            }

            Rectangle {
                visible:
                    root.selectedTrayId.length > 0

                width:
                    visible
                    ? 1
                    : 0

                Layout.fillHeight: true

                color: "#3f334b"
            }

            /*
             * FIXED RIGHT SIDE.
             *
             * This is always against the screen edge.
             */
            ColumnLayout {
                id: fixedColumn

                Layout.preferredWidth: fixedWidth
                Layout.minimumWidth: fixedWidth
                Layout.maximumWidth: fixedWidth

                Layout.fillHeight: true

                spacing: 5

                Label {
                    text: "Modules"

                    color: "#c49be8"

                    font.pixelSize: 11
                    font.bold: true
                }

                ListView {
                    id: trayList

                    Layout.fillWidth: true
                    Layout.preferredHeight:
                        moduleListHeight

                    clip: true

                    interactive:
                        contentHeight > height

                    spacing:
                        moduleSpacing

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
                            ? moduleRowHeight
                            : 0

                        visible:
                            trayVisible

                        radius: 6

                        readonly property bool selected:
                            root.selectedTrayId === trayId

                        color:
                            selected
                            ? "#3a2050"
                            : "#19151f"

                        border.width:
                            selected
                            ? 2
                            : 1

                        border.color:
                            selected
                            ? "#c27cff"
                            : "#3f334b"

                        MouseArea {
                            anchors.fill: parent

                            preventStealing: true
                            acceptedButtons: Qt.LeftButton

                            cursorShape:
                                Qt.PointingHandCursor

                            onClicked: {
                                root.selectedTrayId =
                                    root.selectedTrayId === trayId
                                    ? ""
                                    : trayId
                            }
                        }

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 5

                            spacing: 6

                            Image {
                                source:
                                    icon.length > 0
                                    ? "file://" + icon
                                    : ""

                                width: 26
                                height: 26

                                sourceSize.width: 26
                                sourceSize.height: 26

                                fillMode:
                                    Image.PreserveAspectFit
                            }

                            ColumnLayout {
                                Layout.fillWidth: true

                                spacing: 0

                                Label {
                                    text:
                                        ownerModule

                                    color:
                                        selected
                                        ? "#ffffff"
                                        : "#00e5ff"

                                    font.pixelSize: 11
                                    font.bold: true

                                    elide: Text.ElideRight

                                    Layout.fillWidth: true
                                }

                                Label {
                                    text:
                                        moduleVersion.length > 0
                                        ? "v" + moduleVersion
                                        : ""

                                    color: "#a987c5"

                                    font.pixelSize: 8
                                }
                            }

                            Rectangle {
                                width: 6
                                height: 6
                                radius: 3

                                color:
                                    opened
                                    ? "#9a58d0"
                                    : "#55505a"
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1

                    color: "#3f334b"
                }

                NeeblesButton {
                    id: openBossButton

                    text:
                        root.t("tray.open_boss")

                    Layout.fillWidth: true

                    enabled: false
                }
            }
        }
    }
}
