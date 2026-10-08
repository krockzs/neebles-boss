import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: root

    width: 390
    height: shell.implicitHeight

    visible: false
    title: root.t("tray.title")

    flags: Qt.FramelessWindowHint
    color: "transparent"

    property string selectedModule: ""

    function t(key) {
        if (
            typeof bossStrings !== "undefined"
            && bossStrings[key] !== undefined
        )
            return bossStrings[key]

        return key
    }

    function trayContent(module) {
        if (
            !module
            || !module.surface_content
            || module.surface_content.length === undefined
        )
            return []

        return module.surface_content.filter(
            function(item) {
                return (
                    item
                    && item.surface === "tray"
                    && item.visible
                )
            }
        )
    }

    function trayModules() {
        if (
            typeof bossCommands === "undefined"
            || !bossCommands.installedModules
            || bossCommands.installedModules.length === undefined
        )
            return []

        return bossCommands.installedModules.filter(
            function(module) {
                return (
                    module
                    && module.surface_visibility
                    && module.surface_visibility.tray
                        === true
                    && root.trayContent(module).length > 0
                )
            }
        )
    }

    function selectedModuleObject() {
        const modules =
            root.trayModules()

        for (
            var index = 0;
            index < modules.length;
            ++index
        ) {
            if (
                modules[index].name
                === root.selectedModule
            )
                return modules[index]
        }

        return null
    }

    function selectedContent() {
        return root.trayContent(
            root.selectedModuleObject()
        )
    }

    function selectedButtons() {
        return root.selectedContent().filter(
            function(item) {
                const data =
                    item.data
                    ? item.data
                    : ({})

                return (
                    data.control === "button"
                    && data.action
                    && data.label_key
                )
            }
        )
    }

    function selectedSwitches() {
        return root.selectedContent().filter(
            function(item) {
                const data =
                    item.data
                    ? item.data
                    : ({})

                return (
                    data.control === "switch"
                    && item.object_id
                    && item.active !== undefined
                    && item.active !== null
                    && data.action_on
                    && data.action_off
                    && data.transition_on
                    && data.transition_off
                    && data.label_key
                )
            }
        )
    }

    component NeeblesButton: Button {
        id: control

        implicitHeight: 28
        hoverEnabled: true

        font.pixelSize: 11
        font.bold: true

        contentItem: Text {
            text: control.text
            font: control.font

            color:
                control.enabled
                ? "#67E8F9"
                : "#71717A"

            horizontalAlignment:
                Text.AlignHCenter

            verticalAlignment:
                Text.AlignVCenter

            elide:
                Text.ElideRight
        }

        background: Rectangle {
            radius: 7

            color:
                !control.enabled
                ? "#09090B"
                : control.down
                  ? "#15101E"
                  : control.hovered
                    ? "#1D1430"
                    : "#09090D"

            border.width:
                control.activeFocus
                ? 3
                : 2

            border.color:
                control.enabled
                ? (
                    control.hovered
                    || control.activeFocus
                    ? "#67E8F9"
                    : "#7E22CE"
                )
                : "#3F3F46"
        }
    }

    component NeeblesSwitch: Switch {
        id: control

        implicitWidth: 42
        implicitHeight: 24

        hoverEnabled: true

        indicator: Rectangle {
            anchors.centerIn: parent

            width: 36
            height: 18
            radius: 9

            color:
                control.checked
                ? "#5B21B6"
                : "#27272A"

            border.width:
                control.hovered
                ? 2
                : 1

            border.color:
                control.checked
                ? "#A855F7"
                : control.hovered
                  ? "#67E8F9"
                  : "#3F3F46"

            Rectangle {
                width: 14
                height: 14
                radius: 7

                anchors.verticalCenter:
                    parent.verticalCenter

                x:
                    control.checked
                    ? parent.width - width - 2
                    : 2

                color:
                    control.checked
                    ? "#F5F3FF"
                    : "#A1A1AA"

                Behavior on x {
                    NumberAnimation {
                        duration: 100
                    }
                }
            }
        }

        contentItem: Item {
        }
    }

    onVisibleChanged: {
        if (visible) {
            bossCommands.refreshModules()
        }
    }

    Connections {
        target: bossCommands

        function onInstalledModulesChanged() {
            const modules =
                root.trayModules()

            if (
                root.selectedModule.length > 0
            ) {
                const stillExists =
                    modules.some(
                        function(module) {
                            return (
                                module.name
                                === root.selectedModule
                            )
                        }
                    )

                if (!stillExists)
                    root.selectedModule = ""
            }
        }
    }

    Rectangle {
        id: shell

        anchors.fill: parent

        radius: 10
        color: "#000000"

        border.width: 1
        border.color: "#71449A"

        implicitHeight:
            header.implicitHeight
            + body.implicitHeight
            + 24
    }

    ColumnLayout {
        anchors.fill: shell
        anchors.margins: 8

        spacing: 7

        RowLayout {
            id: header

            Layout.fillWidth: true

            Image {
                source:
                    "qrc:/qt/qml/NEEBLES/TrayHost/neebles-boss-tray-icon.png"

                width: 22
                height: 22

                sourceSize.width: 22
                sourceSize.height: 22

                fillMode:
                    Image.PreserveAspectFit
            }

            Label {
                text:
                    "N.E.E.B.L.E.S."

                color: "#D8B4FE"

                font.pixelSize: 12
                font.bold: true

                Layout.fillWidth: true
            }

            Rectangle {
                width: 7
                height: 7
                radius: 4

                color:
                    trayClient.connected
                    ? "#22D3EE"
                    : "#52525B"
            }
        }

        Label {
            visible:
                trayClient.error.length > 0
                || bossCommands.error.length > 0

            text:
                bossCommands.error.length > 0
                ? bossCommands.error
                : trayClient.error

            color: "#FB7185"

            font.pixelSize: 10

            wrapMode:
                Text.Wrap

            Layout.fillWidth: true
        }

        RowLayout {
            id: body

            Layout.fillWidth: true

            spacing: 7

            Rectangle {
                Layout.preferredWidth: 178
                Layout.minimumWidth: 178
                Layout.maximumWidth: 178

                Layout.preferredHeight: 220

                radius: 7

                color: "#09090D"

                border.width: 1
                border.color: "#3F3F46"

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 7

                    spacing: 5

                    Label {
                        text:
                            root.t(
                                "modules.title"
                            )

                        color: "#C4B5FD"

                        font.pixelSize: 11
                        font.bold: true
                    }

                    ListView {
                        id: moduleList

                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        clip: true

                        spacing: 4

                        model:
                            root.trayModules()

                        delegate: Rectangle {
                            required property var modelData

                            width:
                                moduleList.width

                            height: 38

                            radius: 6

                            readonly property bool selected:
                                root.selectedModule
                                === modelData.name

                            color:
                                selected
                                ? "#211331"
                                : "#111116"

                            border.width:
                                selected
                                ? 2
                                : 1

                            border.color:
                                selected
                                ? "#A855F7"
                                : "#3F3F46"

                            MouseArea {
                                anchors.fill:
                                    parent

                                cursorShape:
                                    Qt.PointingHandCursor

                                onClicked: {
                                    root.selectedModule =
                                        root.selectedModule
                                        === modelData.name
                                        ? ""
                                        : modelData.name
                                }
                            }

                            RowLayout {
                                anchors.fill:
                                    parent

                                anchors.margins: 5

                                spacing: 6

                                Image {
                                    source:
                                        modelData.icon
                                        ? modelData.icon
                                        : ""

                                    width: 25
                                    height: 25

                                    sourceSize.width: 25
                                    sourceSize.height: 25

                                    fillMode:
                                        Image.PreserveAspectFit
                                }

                                Label {
                                    text:
                                        modelData.name

                                    color:
                                        selected
                                        ? "#FFFFFF"
                                        : "#67E8F9"

                                    font.pixelSize: 11
                                    font.bold: true

                                    elide:
                                        Text.ElideRight

                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: "#3F3F46"
                    }

                    NeeblesButton {
                        id: openBossButton

                        Layout.fillWidth: true

                        text:
                            root.t(
                                "tray.open_boss"
                            )

                        enabled:
                            bossEvents.connected
                            && bossEvents.bossUiState
                                === "closed"

                        onClicked: {
                            bossCommands.startBoss()
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 220

                visible:
                    root.selectedModule.length > 0

                radius: 7

                color: "#09090D"

                border.width: 1
                border.color: "#71449A"

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8

                    spacing: 7

                    Label {
                        text:
                            root.selectedModule

                        color: "#D8B4FE"

                        font.pixelSize: 12
                        font.bold: true

                        Layout.fillWidth: true

                        elide:
                            Text.ElideRight
                    }

                    RowLayout {
                        id: selectedModuleRow

                        property var selectedModuleData:
                            root.selectedModuleObject()

                        Layout.fillWidth: true

                        visible:
                            selectedModuleRow.selectedModuleData !== null

                        Label {
                            Layout.fillWidth: true

                            text:
                                selectedModuleRow.selectedModuleData
                                && selectedModuleRow.selectedModuleData.enabled
                                ? root.t("modules.active")
                                : root.t("modules.inactive")

                            color:
                                selectedModuleRow.selectedModuleData
                                && selectedModuleRow.selectedModuleData.enabled
                                ? "#A78BFA"
                                : "#71717A"
                        }

                        NeeblesSwitch {
                            checked:
                                !!selectedModuleRow.selectedModuleData
                                && !!selectedModuleRow.selectedModuleData.enabled

                            checkable: false

                            enabled:
                                !!selectedModuleRow.selectedModuleData
                                && !bossCommands.busy

                            onClicked: {
                                bossCommands.setModuleEnabled(
                                    selectedModuleRow.selectedModuleData.name,
                                    !selectedModuleRow.selectedModuleData.enabled
                                )
                            }
                        }
                    }

                    Repeater {
                        model:
                            root.selectedButtons()

                        delegate: NeeblesButton {
                            required property var modelData

                            property var surfaceItem:
                                modelData

                            property var surfaceData:
                                surfaceItem.data
                                ? surfaceItem.data
                                : ({})

                            Layout.fillWidth: true

                            text:
                                surfaceItem.label

                            enabled:
                                !bossCommands.busy
                                && surfaceItem.action_available
                                === true

                            onClicked: {
                                bossCommands.surfaceAction(
                                    surfaceItem.owner_module,
                                    surfaceItem.item_id,
                                    surfaceData.action
                                )
                            }
                        }
                    }

                    Repeater {
                        model:
                            root.selectedSwitches()

                        delegate: RowLayout {
                            required property var modelData

                            property var surfaceItem:
                                modelData

                            property var surfaceData:
                                surfaceItem.data
                                ? surfaceItem.data
                                : ({})

                            Layout.fillWidth: true

                            Label {
                                Layout.fillWidth: true

                                text:
                                    surfaceItem.label

                                color: "#A78BFA"

                                font.pixelSize: 11

                                elide:
                                    Text.ElideRight
                            }

                            NeeblesSwitch {
                                checked:
                                    !!surfaceItem.active

                                checkable: false

                                enabled:
                                    !bossCommands.busy
                                    && surfaceItem.requirements_met
                                    === true

                                onClicked: {
                                    const turnOn =
                                        !surfaceItem.active

                                    bossCommands.surfaceAction(
                                        surfaceItem.owner_module,
                                        surfaceItem.item_id,
                                        turnOn
                                        ? surfaceData.action_on
                                        : surfaceData.action_off
                                    )
                                }
                            }
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                    }
                }
            }
        }
    }

    Connections {
        target: bossEvents

        function onSettingsChanged() {
            bossCommands.refreshModules()
        }

        function onModulesChanged() {
            bossCommands.refreshModules()
        }
    }

    Component.onCompleted: {
        bossCommands.refreshModules()
    }
}
