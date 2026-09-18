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
    property string bossVersion: "1.0.14"

    LauncherBossEvents {
        id: bossEvents

        Component.onCompleted:
            connectToBoss()

        onHiddenLauncherModulesChanged:
            root.applyModuleFilter()
    }

    function safeModuleId(value) {
        return /^[A-Za-z0-9._-]+$/.test(value)
    }

    function exec(command, callback) {
        callbacks[command] = callback
        runner.connectSource(command)
    }

    function applyModuleFilter() {
        modules = installedModules.filter(
            function(module) {
                return (
                    typeof module.launcher_action === "string"
                    && module.launcher_action.length > 0
                    && root.safeModuleId(module.name)
                    && root.safeModuleId(
                        module.launcher_action
                    )
                    && bossEvents.hiddenLauncherModules.indexOf(
                        module.name
                    ) === -1
                )
            }
        )
    }

    function refresh() {
        exec("neebles --version", function(output) {
            const value = output.trim()
            const match = value.match(/([0-9]+\.[0-9]+\.[0-9]+)/)

            if (match)
                bossVersion = match[1]
        })

        exec("neebles i18n dump", function(output) {
            try {
                strings = JSON.parse(output)
            } catch (e) {
                strings = ({})
            }
        })

        exec("neebles modules installed", function(output) {
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
                178 + Math.max(1, root.modules.length) * 58
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

                        width: ListView.view.width
                        height: 52
                        radius: 8

                        color: "#09090D"

                        border.width: 1
                        border.color:
                            modelData.enabled
                            ? "#4C1D95"
                            : "#3F3F46"

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 7

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
                                id: openButton

                                Layout.preferredWidth: 54
                                Layout.preferredHeight: 28

                                text:
                                    root.t(
                                        "common.open"
                                    )

                                enabled:
                                    modelData.enabled
                                    && root.safeModuleId(
                                        modelData.name
                                    )
                                    && root.safeModuleId(
                                        modelData.launcher_action
                                    )

                                background: Rectangle {
                                    radius: 7

                                    color:
                                        openButton.enabled
                                        ? "#0B1015"
                                        : "#202024"

                                    border.width:
                                        openButton.enabled
                                        ? 2
                                        : 1

                                    border.color:
                                        openButton.enabled
                                        ? "#22D3EE"
                                        : "#52525B"
                                }

                                contentItem: Text {
                                    text: openButton.text

                                    color:
                                        openButton.enabled
                                        ? "#67E8F9"
                                        : "#71717A"

                                    font.pixelSize: 11
                                    font.bold: true

                                    horizontalAlignment:
                                        Text.AlignHCenter

                                    verticalAlignment:
                                        Text.AlignVCenter
                                }

                                onClicked:
                                    root.exec(
                                        "neebles "
                                        + modelData.name
                                        + " "
                                        + modelData.launcher_action,
                                        function() {}
                                    )
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

                                background: Rectangle {
                                    radius: 7
                                    color: "#110B19"

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
                                        "neebles modules "
                                        + verb
                                        + " "
                                        + modelData.name,
                                        function() {
                                            root.refresh()
                                        }
                                    )
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
                        "neebles start",
                        function() {}
                    )
            }
        }
    }
}
