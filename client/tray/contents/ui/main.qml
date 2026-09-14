import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.plasma.plasmoid
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    hideOnWindowDeactivate: true

    property bool trayEnabled: true

    Plasmoid.icon:
        Qt.resolvedUrl(
            "../images/neebles-boss-launcher-icon.png"
        )

    property var modules: []
    property var strings: ({})
    property var callbacks: ({})
    property var hiddenTrayModules: []

    property string selectedModule: ""

    readonly property var selectedModuleData: {
        for (let i = 0; i < modules.length; ++i) {
            if (modules[i].name === selectedModule)
                return modules[i]
        }

        return null
    }

    function safeModuleId(value) {
        return /^[A-Za-z0-9._-]+$/.test(value)
    }

    function exec(command, callback) {
        callbacks[command] = callback || function() {}
        runner.connectSource(command)
    }

    function t(key, fallback) {
        return strings[key] || fallback
    }

    /*
     * Primero resolvemos configuración y DESPUÉS módulos.
     *
     * Así no repetimos la carrera que existe actualmente
     * en el launcher entre hidden_* y modules installed.
     */
    function refresh() {
        exec("neebles i18n dump", function(output) {
            try {
                strings = JSON.parse(output)
            } catch (e) {
                strings = ({})
            }
        })

        exec("neebles config show", function(output) {
            try {
                const cfg = JSON.parse(output)

                trayEnabled =
                    cfg.tray_enabled !== false

                hiddenTrayModules =
                    Array.isArray(cfg.hidden_tray_modules)
                    ? cfg.hidden_tray_modules
                    : []
            } catch (e) {
                trayEnabled = true
                hiddenTrayModules = []
            }

            refreshModules()
        })
    }

    function refreshModules() {
        exec("neebles modules installed", function(output) {
            try {
                const installed = JSON.parse(output)

                modules = installed.filter(function(module) {
                    return root.hiddenTrayModules.indexOf(
                        module.name
                    ) === -1
                })

                if (selectedModule !== "") {
                    let found = false

                    for (let i = 0; i < modules.length; ++i) {
                        if (modules[i].name === selectedModule) {
                            found = true
                            break
                        }
                    }

                    if (!found)
                        selectedModule = ""
                }
            } catch (e) {
                modules = []
                selectedModule = ""
            }
        })
    }

    function openModule(name) {
        if (!safeModuleId(name))
            return

        exec(
            "neebles " + name + " open",
            function() {}
        )
    }

    function setModuleEnabled(name, enabled) {
        if (!safeModuleId(name))
            return

        const action =
            enabled
            ? "enable"
            : "disable"

        exec(
            "neebles modules " + action + " " + name,
            function() {
                refreshModules()
            }
        )
    }

    Component.onCompleted:
        refresh()

    /*
     * Sólo refresca mientras el tray está abierto.
     * No necesitamos estar consultando módulos eternamente.
     */

    Plasma5Support.DataSource {
        id: runner

        engine: "executable"

        onNewData: function(sourceName, data) {
            const callback =
                root.callbacks[sourceName]

            if (callback)
                callback(data.stdout || "")

            delete root.callbacks[sourceName]

            disconnectSource(sourceName)
        }
    }

    /*
     * POPUP.
     *
     * NO ES UNA WINDOW.
     * Plasma crea y administra la ventana contenedora.
     */
    fullRepresentation: Item {
        Layout.minimumWidth:
            root.selectedModuleData
            ? 430
            : 235

        Layout.preferredWidth:
            root.selectedModuleData
            ? 430
            : 235

        Layout.maximumWidth:
            root.selectedModuleData
            ? 430
            : 235

        Layout.minimumHeight:
            Math.max(
                90,
                leftColumn.implicitHeight + 10
            )

        Layout.preferredHeight:
            Math.max(
                90,
                leftColumn.implicitHeight + 10
            )

        implicitWidth:
            root.selectedModuleData
            ? 430
            : 235

        implicitHeight:
            Math.max(
                90,
                leftColumn.implicitHeight + 10
            )

        RowLayout {
            anchors.fill: parent
            anchors.margins: 5

            spacing: 5

            /*
             * IZQUIERDA
             */
            Rectangle {
                Layout.preferredWidth: 225
                Layout.fillHeight: true

                radius: 8

                color: "#030305"

                border.width: 1
                border.color: "#A855F7"

                ColumnLayout {
                    id: leftColumn

                    anchors.fill: parent
                    anchors.margins: 5

                    spacing: 4

                    /*
                     * SR. NEEBLES + TRAY
                     */
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 34

                        spacing: 7

                        Image {
                            Layout.preferredWidth: 30
                            Layout.preferredHeight: 30

                            source:
                                "../images/neebles-boss-launcher-icon.png"

                            fillMode:
                                Image.PreserveAspectFit

                            smooth: true
                            mipmap: true
                        }

                        Text {
                            Layout.fillWidth: true

                            text:
                                t(
                                    "tray.title",
                                    "Tray"
                                )

                            color: "#D8B4FE"

                            font.pixelSize: 13
                            font.bold: true
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1

                        color: "#4C1D95"
                    }

                    Text {
                        visible:
                            root.modules.length === 0

                        Layout.fillWidth: true
                        Layout.preferredHeight: 32

                        text:
                            t(
                                "modules.empty",
                                "No modules are installed."
                            )

                        color: "#71717A"

                        font.pixelSize: 9

                        verticalAlignment:
                            Text.AlignVCenter

                        horizontalAlignment:
                            Text.AlignHCenter
                    }

                    Repeater {
                        model:
                            root.modules

                        delegate: Rectangle {
                            id: moduleSelect

                            required property var modelData

                            property bool selected:
                                root.selectedModule
                                === modelData.name

                            Layout.fillWidth: true
                            Layout.preferredHeight: 38

                            radius: 6

                            color:
                                selected
                                ? "#1B1027"
                                : modelData.enabled
                                  ? "#07070A"
                                  : "#101014"

                            border.width: 1

                            border.color:
                                selected
                                ? "#A855F7"
                                : modelData.enabled
                                  ? "#22D3EE"
                                  : "#52525B"

                            RowLayout {
                                anchors.fill: parent
                                anchors.margins: 4

                                spacing: 6

                                Rectangle {
                                    Layout.preferredWidth: 28
                                    Layout.preferredHeight: 28

                                    radius: 6

                                    color: "#050507"

                                    border.width: 1

                                    border.color:
                                        modelData.enabled
                                        ? "#22D3EE"
                                        : "#52525B"

                                    Image {
                                        anchors.fill: parent
                                        anchors.margins: 3

                                        source:
                                            modelData.icon || ""

                                        fillMode:
                                            Image.PreserveAspectFit

                                        smooth: true
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true

                                    spacing: 0

                                    Text {
                                        Layout.fillWidth: true

                                        text:
                                            modelData.name

                                        color:
                                            modelData.enabled
                                            ? "#67E8F9"
                                            : "#71717A"

                                        font.pixelSize: 10
                                        font.bold: true

                                        elide:
                                            Text.ElideRight
                                    }

                                    Text {
                                        text:
                                            modelData.version

                                        color:
                                            modelData.enabled
                                            ? "#A78BFA"
                                            : "#52525B"

                                        font.pixelSize: 8
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent

                                onClicked: {
                                    root.selectedModule =
                                        moduleSelect.selected
                                        ? ""
                                        : modelData.name
                                }
                            }
                        }
                    }
                }
            }

            /*
             * DERECHA: SÓLO EL MÓDULO SELECCIONADO
             */
            Rectangle {
                visible:
                    root.selectedModuleData !== null

                Layout.preferredWidth: 190
                Layout.fillHeight: true

                radius: 7

                color: "#08080C"

                border.width: 1
                border.color: "#A855F7"

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8

                    spacing: 7

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 36

                        spacing: 7

                        Rectangle {
                            Layout.preferredWidth: 32
                            Layout.preferredHeight: 32

                            radius: 7

                            color: "#050507"

                            border.width: 1

                            border.color:
                                root.selectedModuleData
                                && root.selectedModuleData.enabled
                                ? "#22D3EE"
                                : "#52525B"

                            Image {
                                anchors.fill: parent
                                anchors.margins: 3

                                source:
                                    root.selectedModuleData
                                    ? root.selectedModuleData.icon || ""
                                    : ""

                                fillMode:
                                    Image.PreserveAspectFit

                                smooth: true
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true

                            spacing: 0

                            Text {
                                Layout.fillWidth: true

                                text:
                                    root.selectedModuleData
                                    ? root.selectedModuleData.name
                                    : ""

                                color:
                                    root.selectedModuleData
                                    && root.selectedModuleData.enabled
                                    ? "#67E8F9"
                                    : "#71717A"

                                font.pixelSize: 11
                                font.bold: true

                                elide:
                                    Text.ElideRight
                            }

                            Text {
                                text:
                                    root.selectedModuleData
                                    ? root.selectedModuleData.version
                                    : ""

                                color:
                                    root.selectedModuleData
                                    && root.selectedModuleData.enabled
                                    ? "#A78BFA"
                                    : "#52525B"

                                font.pixelSize: 8
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1

                        color: "#4C1D95"
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        /*
                         * OPEN
                         */
                        Rectangle {
                            Layout.preferredWidth: 55
                            Layout.preferredHeight: 24

                            radius: 5

                            property bool usable:
                                root.selectedModuleData
                                && root.selectedModuleData.enabled

                            color:
                                usable
                                ? "#07161A"
                                : "#111113"

                            border.width: 1

                            border.color:
                                usable
                                ? "#22D3EE"
                                : "#3F3F46"

                            Text {
                                anchors.centerIn: parent

                                text:
                                    t(
                                        "common.open",
                                        "Open"
                                    )

                                color:
                                    parent.usable
                                    ? "#67E8F9"
                                    : "#52525B"

                                font.pixelSize: 9
                            }

                            MouseArea {
                                anchors.fill: parent

                                enabled:
                                    parent.usable

                                onClicked:
                                    root.openModule(
                                        root.selectedModule
                                    )
                            }
                        }

                        Item {
                            Layout.fillWidth: true
                        }

                        /*
                         * SWITCH + ACTIVATE
                         */
                        ColumnLayout {
                            spacing: 0

                            Rectangle {
                                id: switchTrack

                                Layout.alignment:
                                    Qt.AlignHCenter

                                Layout.preferredWidth: 30
                                Layout.preferredHeight: 14

                                radius: 7

                                property bool checked:
                                    root.selectedModuleData
                                    ? root.selectedModuleData.enabled
                                    : false

                                color:
                                    checked
                                    ? "#4C1D95"
                                    : "#27272A"

                                border.width: 1

                                border.color:
                                    checked
                                    ? "#A855F7"
                                    : "#52525B"

                                Rectangle {
                                    width: 10
                                    height: 10

                                    radius: 5

                                    y: 1

                                    x:
                                        switchTrack.checked
                                        ? parent.width - width - 2
                                        : 2

                                    color:
                                        switchTrack.checked
                                        ? "#D8B4FE"
                                        : "#71717A"

                                    Behavior on x {
                                        NumberAnimation {
                                            duration: 100
                                        }
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent

                                    onClicked:
                                        root.setModuleEnabled(
                                            root.selectedModule,
                                            !switchTrack.checked
                                        )
                                }
                            }

                            Text {
                                Layout.alignment:
                                    Qt.AlignHCenter

                                text:
                                    t(
                                        "tray.activate",
                                        "Activate"
                                    )

                                color: "#A1A1AA"

                                font.pixelSize: 8
                            }
                        }
                    }
                }
            }
        }
    }
}
