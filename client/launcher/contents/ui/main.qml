import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    property bool launcherEnabled: true
    property var modules: []
    property var strings: ({})
    property var callbacks: ({})
    property bool bossRunning: false

    function safeModuleId(value) {
        return /^[A-Za-z0-9._-]+$/.test(value)
    }

    function exec(command, callback) {
        callbacks[command] = callback
        runner.connectSource(command)
    }

    function refreshRuntimeState() {
        exec("neebles config show", function(output) {
            try {
                const cfg = JSON.parse(output)
                launcherEnabled = cfg.launcher_enabled !== false
            } catch (e) {
                launcherEnabled = true
            }
        })

    }

    function refresh() {
        refreshRuntimeState()

        exec("neebles i18n dump", function(output) {
            try { strings = JSON.parse(output) } catch (e) { strings = ({}) }
        })
        exec("neebles modules installed", function(output) {
            try { modules = JSON.parse(output) } catch (e) { modules = [] }
        })
        exec("qdbus6 org.freedesktop.DBus / org.freedesktop.DBus.NameHasOwner org.neebles.Boss", function(output) {
            bossRunning = output.trim() === "true"
        })
    }

    function t(key, fallback) {
        return strings[key] || fallback
    }

    onExpandedChanged: {
        if (expanded)
            refresh()
    }

    Component.onCompleted: refresh()

    Timer {
        interval: 1500
        running: true
        repeat: true
        onTriggered: root.refreshRuntimeState()
    }

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
        implicitWidth: root.launcherEnabled ? 38 : 0
        implicitHeight: 38
        visible: root.launcherEnabled

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
        implicitWidth: 320
        implicitHeight: Math.max(160, moduleColumn.implicitHeight + 32)

        ColumnLayout {
            id: moduleColumn
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            PlasmaComponents3.Label {
                text: "N.E.E.B.L.E.S."
                font.bold: true
                font.pixelSize: 18
            }

            PlasmaComponents3.Label {
                visible: root.modules.length === 0
                text: root.t("modules.empty", "No modules are installed.")
                opacity: 0.7
            }

            Repeater {
                model: root.modules

                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 8

                    PlasmaComponents3.Label {
                        Layout.fillWidth: true
                        text: modelData.name
                        elide: Text.ElideRight
                    }

                    PlasmaComponents3.Button {
                        text: root.t("common.open", "Open")
                        enabled: modelData.enabled && root.safeModuleId(modelData.name)
                        onClicked: root.exec("neebles " + modelData.name + " open", function() {})
                    }

                    PlasmaComponents3.Button {
                        text: modelData.enabled
                              ? root.t("common.disable", "Disable")
                              : root.t("common.enable", "Enable")
                        enabled: root.safeModuleId(modelData.name)
                        onClicked: {
                            const verb = modelData.enabled ? "disable" : "enable"
                            root.exec("neebles modules " + verb + " " + modelData.name, function() { root.refresh() })
                        }
                    }
                }
            }

            Item { Layout.fillHeight: true }

            PlasmaComponents3.Button {
                Layout.fillWidth: true
                text: root.t("launcher.open_boss", "Open Boss")
                enabled: !root.bossRunning
                onClicked: root.exec("neebles start", function() {})
            }
        }
    }
}
