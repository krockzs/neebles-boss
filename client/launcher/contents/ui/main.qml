import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.plasma5support as Plasma5Support
import NEEBLES.BossEvents 1.0

PlasmoidItem {
    id: root

    property var modules: []
    property var installedModules: []
    property var strings: ({})
    property var callbacks: ({})
    property string bossVersion: "1.0.29"

    component NeeblesSwitch: QQC2.Switch {
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
            }
        }

        contentItem: Item {
        }
    }


    LauncherBossEvents {
        id: bossEvents

        Component.onCompleted:
            connectToBoss()

        onSettingsChanged:
            root.refresh()

        onModulesChanged:
            root.refresh()
    }

    /*
     * Keep the applet physically provisioned at all times.
     * Global Launcher OFF collapses only its visual panel footprint.
     */
    Layout.minimumWidth:
        bossEvents.launcherEnabled ? 38 : 0

    Layout.preferredWidth:
        bossEvents.launcherEnabled ? 38 : 0

    Layout.maximumWidth:
        bossEvents.launcherEnabled ? 38 : 0

    function safeModuleId(value) {
        return /^[A-Za-z0-9._-]+$/.test(value)
    }

    function exec(command, callback) {
        callbacks[command] = callback
        runner.connectSource(command)
    }

    function launcherContent(module) {
        if (
            !module
            || !Array.isArray(module.surface_content)
        )
            return []

        return module.surface_content.filter(
            function(item) {
                return (
                    item
                    && item.surface === "launcher"
                    && item.visible
                )
            }
        )
    }

    function launcherActionButtons(module) {
        return launcherContent(module).filter(
            function(item) {
                const data =
                    item.data
                    ? item.data
                    : ({})

                return (
                    data.control === "button"
                    && typeof data.action === "string"
                    && data.action.length > 0
                    && typeof data.label_key === "string"
                    && data.label_key.length > 0
                )
            }
        )
    }

    function launcherStateSwitches(module) {
        return launcherContent(module).filter(
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

    function applyModuleFilter() {
        modules = installedModules.filter(
            function(module) {
                return (
                    root.safeModuleId(module.name)
                    && root.launcherContent(module).length > 0
                )
            }
        )
    }

    function shellArg(value) {
        return "'"
            + String(value).replace(
                /'/g,
                "'\\''"
            )
            + "'"
    }

    function surfaceActionCommand(
        moduleName,
        itemId,
        action
    ) {
        return "/opt/neebles/client/bin/neebles boss surface-action "
            + shellArg(moduleName)
            + " "
            + shellArg(itemId)
            + " "
            + shellArg(action)
    }
    function refresh() {
        exec("/opt/neebles/client/bin/neebles --version", function(output) {
            const value = output.trim()
            const match = value.match(/([0-9]+\.[0-9]+\.[0-9]+)/)

            if (match)
                bossVersion = match[1]
        })

        exec("/opt/neebles/client/bin/neebles i18n dump", function(output) {
            try {
                strings = JSON.parse(output)
            } catch (e) {
                strings = ({})
            }
        })

        exec("/opt/neebles/client/bin/neebles boss surface-model", function(output) {
            try {
                const installed = JSON.parse(output)

                installedModules =
                    Array.isArray(installed)
                    ? installed
                    : []

                applyModuleFilter()
            } catch (e) {
                installedModules = []
                modules = []
            }
        })

    }

    function t(key) {
        return strings[key] || key
    }

    onExpandedChanged: {
        if (expanded)
            refresh()
    }

    Component.onCompleted: refresh()

    Plasma5Support.DataSource {
        id: runner
        engine: "executable"

        onNewData: function(sourceName, data) {
            const callback = root.callbacks[sourceName]

            if (callback)
                callback(data.stdout || "")

            delete root.callbacks[sourceName]
            disconnectSource(sourceName)
        }
    }

    preferredRepresentation: compactRepresentation

    compactRepresentation: Item {
        implicitWidth: bossEvents.launcherEnabled ? 38 : 0
        implicitHeight: 38
        visible: bossEvents.launcherEnabled

        Image {
            anchors.fill: parent
            source: "../images/neebles-boss-launcher-icon.png"
            fillMode: Image.PreserveAspectFit
            smooth: true
        }

        MouseArea {
            anchors.fill: parent
            onClicked: root.expanded = !root.expanded
        }
    }

    fullRepresentation: Item {
        id: panel

        implicitWidth: 350

        implicitHeight: Math.min(
            470,
            Math.max(
                250,
                178 + Math.max(58, moduleList.contentHeight)
            )
        )

        /*
         * Halo exterior.
         * No afecta el contenido ni la lógica.
         */
        Rectangle {
            anchors.fill: parent
            radius: 18
            color: "transparent"

            border.width: 5
            border.color: "#36205F"
            opacity: 0.55
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            radius: 15

            color: "#030305"

            border.width: 2
            border.color: "#7C3AED"
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: 6
            radius: 12
            color: "transparent"

            border.width: 1
            border.color: "#C084FC"
            opacity: 0.55
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16

            spacing: 10

            /*
             * BRANDING
             */
            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 94

                spacing: 1

                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 74

                    Image {
                        anchors.centerIn: parent
                        width: 74
                        height: 74

                    source:
                        "../images/neebles-boss-launcher-icon.png"

                        fillMode: Image.PreserveAspectFit
                        smooth: true
                        mipmap: true
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter

                    text: "v" + root.bossVersion

                    color: "#A78BFA"
                    font.pixelSize: 12
                    font.bold: true
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1

                color: "#4C1D95"
                opacity: 0.9
            }

            /*
             * ZONA DE MÓDULOS.
             *
             * Es la única zona que puede hacer scroll.
             * Nunca desplaza el botón Open Boss.
             */
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumHeight: 54

                ListView {
                    id: moduleList

                    anchors.fill: parent

                    clip: true
                    spacing: 7

                    model: root.modules

                    boundsBehavior: Flickable.StopAtBounds

                    delegate: Rectangle {
                        required property var modelData

                        property var launcherButtons:
                            root.launcherActionButtons(
                                modelData
                            )

                        property var launcherSwitches:
                            root.launcherStateSwitches(
                                modelData
                            )

                        width:
                            ListView.view.width

                        height:
                            48
                            + launcherButtons.length * 36
                            + launcherSwitches.length * 34

                        radius: 8
                        color: "#09090D"

                        border.width: 1

                        border.color:
                            modelData.enabled
                            ? "#4C1D95"
                            : "#3F3F46"

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 7

                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 28

                                spacing: 7

                                Text {
                                    Layout.fillWidth: true

                                    text: modelData.name

                                    color:
                                        modelData.enabled
                                        ? "#F5F3FF"
                                        : "#71717A"

                                    font.pixelSize: 13
                                    font.bold: true

                                    elide: Text.ElideRight
                                }

                                PlasmaComponents3.Button {
                                    id: stateButton

                                    Layout.preferredWidth: 64
                                    Layout.preferredHeight: 28

                                    text:
                                        modelData.enabled
                                        ? root.t(
                                            "common.disable"
                                        )
                                        : root.t(
                                            "common.enable"
                                        )

                                    enabled:
                                        root.safeModuleId(
                                            modelData.name
                                        )

                                    hoverEnabled: true

                                    background: Rectangle {
                                        radius: 7

                                        color:
                                            stateButton.down
                                            ? "#1B1027"
                                            : stateButton.hovered
                                              ? "#211331"
                                              : "#110B19"

                                        border.width: 2

                                        border.color:
                                            modelData.enabled
                                            ? "#A855F7"
                                            : "#22D3EE"
                                    }

                                    contentItem: Text {
                                        text: stateButton.text

                                        color:
                                            modelData.enabled
                                            ? "#D8B4FE"
                                            : "#67E8F9"

                                        font.pixelSize: 12
                                        font.bold: true

                                        horizontalAlignment:
                                            Text.AlignHCenter

                                        verticalAlignment:
                                            Text.AlignVCenter
                                    }

                                    onClicked: {
                                        const verb =
                                            modelData.enabled
                                            ? "disable"
                                            : "enable"

                                        root.exec(
                                            "/opt/neebles/client/bin/neebles modules "
                                            + verb
                                            + " "
                                            + root.shellArg(
                                                modelData.name
                                            ),
                                            function() {
                                                root.refresh()
                                            }
                                        )
                                    }
                                }
                            }

                            Repeater {
                                model:
                                    launcherButtons

                                delegate: PlasmaComponents3.Button {
                                    required property var modelData

                                    property var surfaceItem:
                                        modelData

                                    property var surfaceData:
                                        surfaceItem.data
                                        ? surfaceItem.data
                                        : ({})

                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 30

                                    text:
                                        surfaceItem.label

                                    enabled:
                                        surfaceItem.requirements_met
                                        === true

                                    hoverEnabled: true

                                    background: Rectangle {
                                        radius: 7

                                        color:
                                            parent.down
                                            ? "#0B1220"
                                            : parent.hovered
                                              ? "#172033"
                                              : "#10131A"

                                        border.width:
                                            parent.activeFocus
                                            ? 3
                                            : 2

                                        border.color:
                                            parent.hovered
                                            || parent.activeFocus
                                            ? "#67E8F9"
                                            : "#22D3EE"
                                    }

                                    contentItem: Text {
                                        text: parent.text
                                        color: "#67E8F9"

                                        font.pixelSize: 11
                                        font.bold: true

                                        horizontalAlignment:
                                            Text.AlignHCenter

                                        verticalAlignment:
                                            Text.AlignVCenter
                                    }

                                    onClicked: {
                                        root.exec(
                                            root.surfaceActionCommand(
                                                surfaceItem.owner_module,
                                                surfaceItem.item_id,
                                                surfaceData.action
                                            ),
                                            function() {
                                                root.refresh()
                                            }
                                        )
                                    }
                                }
                            }

                            Repeater {
                                model:
                                    launcherSwitches

                                delegate: RowLayout {
                                    required property var modelData

                                    property var surfaceItem:
                                        modelData

                                    property var surfaceData:
                                        surfaceItem.data
                                        ? surfaceItem.data
                                        : ({})

                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 28

                                    Text {
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
                                            surfaceItem.requirements_met
                                            === true

                                        onClicked: {
                                            const turnOn =
                                                !surfaceItem.active

                                            root.exec(
                                                root.surfaceActionCommand(
                                                    surfaceItem.owner_module,
                                                    surfaceItem.item_id,
                                                    turnOn
                                                    ? surfaceData.action_on
                                                    : surfaceData.action_off
                                                ),
                                                function() {
                                                    root.refresh()
                                                }
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }

                    QQC2.ScrollBar.vertical: QQC2.ScrollBar {
                        policy:
                            moduleList.contentHeight
                            > moduleList.height
                            ? QQC2.ScrollBar.AsNeeded
                            : QQC2.ScrollBar.AlwaysOff
                    }
                }

                Text {
                    anchors.centerIn: parent

                    visible:
                        root.modules.length === 0

                    text:
                        root.t(
                            "modules.empty"
                        )

                    color: "#71717A"
                    font.pixelSize: 12
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1

                color: "#27272A"
            }

            /*
             * OPEN BOSS SIEMPRE FIJO ABAJO.
             */
            PlasmaComponents3.Button {
                id: openBossButton

                Layout.fillWidth: true
                Layout.preferredHeight: 42

                text:
                    root.t(
                        "launcher.open_boss"
                    )

                enabled:
                    bossEvents.connected
                    && bossEvents.bossUiState === "closed"

                background: Item {
                    Rectangle {
                        anchors.fill: parent
                        radius: 9

                        color:
                            openBossButton.enabled
                            ? "#120A1D"
                            : "#202024"

                        border.width:
                            openBossButton.enabled
                            ? 4
                            : 1

                        border.color:
                            openBossButton.enabled
                            ? "#3B176F"
                            : "#52525B"

                        opacity:
                            openBossButton.enabled
                            ? 0.65
                            : 1
                    }

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins:
                            openBossButton.enabled
                            ? 2
                            : 0

                        radius: 8

                        color:
                            openBossButton.enabled
                            ? "#090B12"
                            : "#202024"

                        border.width:
                            openBossButton.enabled
                            ? 2
                            : 1

                        border.color:
                            openBossButton.enabled
                            ? "#22D3EE"
                            : "#52525B"
                    }
                }

                contentItem: Text {
                    text: openBossButton.text

                    color:
                        openBossButton.enabled
                        ? "#D8B4FE"
                        : "#71717A"

                    font.pixelSize: 13
                    font.bold: true

                    horizontalAlignment:
                        Text.AlignHCenter

                    verticalAlignment:
                        Text.AlignVCenter
                }

                onClicked:
                    root.exec(
                        "/opt/neebles/client/bin/neebles start",
                        function() {}
                    )
            }
        }
    }
}
