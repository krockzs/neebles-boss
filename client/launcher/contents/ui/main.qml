import QtQuick
import QtQuick.Layouts
import QtQuick.Window
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
    property var pendingActions: ({})
    property bool refreshBusy: false
    property bool refreshAgain: false
    property string commandError: ""
    property string bossVersion: "1.0.34"

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

    // Commands are keyed by the DataSource source string. Never overwrite
    // an in-flight callback for that same command: that loses completions.
    function exec(command, callback) {
        if (callbacks[command] !== undefined)
            return false
        callbacks[command] = callback
        runner.connectSource(command)
        return true
    }

    function actionError(data) {
        if (!data)
            return ""
        const code = data["exit code"] !== undefined
                     ? data["exit code"] : data.exitCode
        const failure = code !== undefined && Number(code) !== 0
        const stderr = data.stderr ? String(data.stderr).trim() : ""
        return failure ? (stderr || "exit code: " + String(code)) : ""
    }

    // Pending controls are per module. Opening module A must never block B.
    function modulePending(moduleName) {
        return pendingActions[moduleName] === true
    }

    function setModulePending(moduleName, value) {
        const next = Object.assign({}, pendingActions)
        if (value)
            next[moduleName] = true
        else
            delete next[moduleName]
        pendingActions = next
    }

    // Called from root, never from an asynchronous delegate closure.
    function runModuleAction(command, moduleName) {
        if (modulePending(moduleName))
            return
        setModulePending(moduleName, true)
        commandError = ""
        if (!exec(command, function(_stdout, data) {
            setModulePending(moduleName, false)
            commandError = actionError(data)
            if (commandError.length > 0)
                console.warn("N.E.E.B.L.E.S. Launcher action failed:", commandError)
            refresh()
        })) {
            setModulePending(moduleName, false)
            refresh()
        }
    }

    function launcherContent(module) {
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
                    && module.surface_visibility
                    && module.surface_visibility.launcher
                        === true
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
    function loadStaticData() {
        exec("/opt/neebles/client/bin/neebles --version", function(output) {
            const match = output.trim().match(/([0-9]+\.[0-9]+\.[0-9]+)/)
            if (match)
                bossVersion = match[1]
        })
        exec("/opt/neebles/client/bin/neebles i18n dump", function(output) {
            try {
                strings = JSON.parse(output)
            } catch (e) {
                console.warn("N.E.E.B.L.E.S. Launcher i18n refresh error:", String(e))
            }
        })
    }

    // Collapse overlapping event/poll requests into one canonical refresh
    // followed by one more read when events arrive during that read.
    function refresh() {
        if (refreshBusy) {
            refreshAgain = true
            return
        }
        refreshBusy = true
        if (!exec("/opt/neebles/client/bin/neebles boss surface-model", function(output, data) {
            try {
                const error = actionError(data)
                if (error.length > 0)
                    throw new Error(error)
                const installed = JSON.parse(output)
                if (!Array.isArray(installed))
                    throw new Error("Surface model is not an array")
                installedModules = installed
                applyModuleFilter()
            } catch (e) {
                // Never erase a valid model because one refresh failed.
                console.warn("N.E.E.B.L.E.S. Launcher canonical refresh failed:", String(e))
            } finally {
                refreshBusy = false
                if (refreshAgain) {
                    refreshAgain = false
                    refresh()
                }
            }
        })) {
            refreshBusy = false
            refreshAgain = true
        }
    }

    function t(key) {
        return strings[key] || key
    }

    onExpandedChanged: {
        if (expanded)
            refresh()
    }

    Component.onCompleted: {
        loadStaticData()
        refresh()
    }

    // Recover canonical state even if Plasma misses an event subscription.
    Timer {
        interval: 800
        repeat: true
        running: root.expanded && bossEvents.launcherEnabled
        onTriggered: root.refresh()
    }

    Plasma5Support.DataSource {
        id: runner
        engine: "executable"

        onNewData: function(sourceName, data) {
            const callback = root.callbacks[sourceName]
            // Clean up first: an exception in user QML must not leak a source.
            delete root.callbacks[sourceName]
            disconnectSource(sourceName)
            if (callback) {
                try {
                    callback(data.stdout || "", data)
                } catch (e) {
                    console.warn("N.E.E.B.L.E.S. Launcher callback:", String(e))
                    root.pendingActions = ({})
                    root.refreshBusy = false
                }
            }
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

        // Give declared module controls the main share of the popup.
        // The list remains scrollable; Open Boss stays below it.
        // At most 25% wider than Gate148; double the original 470px height.
        // On a shorter display Plasma uses the available screen height.
        implicitWidth: 512
        implicitHeight: Math.min(940, Math.max(340, Screen.availableHeight - 72))

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
                Layout.minimumHeight: 150

                ListView {
                    id: moduleList

                    anchors.fill: parent

                    clip: true
                    spacing: 7

                    model: root.modules

                    boundsBehavior: Flickable.StopAtBounds

                    delegate: Rectangle {
                        id: moduleRow

                        required property var modelData

                        // Keep the two mandatory identity columns. The three
                        // action slots do not create undeclared module actions.
                        readonly property var declaredButtons:
                            root.launcherActionButtons(modelData)
                        readonly property var declaredSwitches:
                            root.launcherStateSwitches(modelData)
                        readonly property var declaredOpen:
                            declaredButtons.find(function(item) {
                                return item.data && item.data.action === "open"
                            }) || null
                        readonly property var remainingButtons:
                            declaredButtons.filter(function(item) {
                                return item !== declaredOpen
                            })
                        readonly property int extraCount:
                            remainingButtons.length + declaredSwitches.length

                        width: ListView.view.width
                        height: Math.max(58, 14 + extraCount * 30)
                        radius: 8
                        color: "#09090D"
                        border.width: 1
                        border.color: modelData.enabled ? "#4C1D95" : "#3F3F46"

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8
                            anchors.topMargin: 6
                            anchors.bottomMargin: 6
                            spacing: 7

                            // Column 1 — mandatory module icon.
                            Image {
                                Layout.preferredWidth: 36
                                Layout.preferredHeight: 36
                                Layout.alignment: Qt.AlignVCenter
                                source: moduleRow.modelData.icon
                                        && moduleRow.modelData.icon.length > 0
                                        ? moduleRow.modelData.icon
                                        : "../images/neebles-boss-launcher-icon.png"
                                fillMode: Image.PreserveAspectFit
                                smooth: true
                            }

                            // Column 2 — mandatory name, up to two lines.
                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 110
                                Layout.alignment: Qt.AlignVCenter
                                text: moduleRow.modelData.name
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                color: moduleRow.modelData.enabled
                                       ? "#F5F3FF" : "#71717A"
                                font.pixelSize: 12
                                font.bold: true
                            }

                            // Column 3 — existing administrative Active control.
                            Item {
                                Layout.preferredWidth: 76
                                Layout.fillHeight: true

                                PlasmaComponents3.Button {
                                    id: stateButton
                                    anchors.centerIn: parent
                                    width: 76
                                    height: 29
                                    text: moduleRow.modelData.enabled
                                          ? root.t("common.disable")
                                          : root.t("common.enable")
                                    enabled: !root.modulePending(moduleRow.modelData.name)
                                             && root.safeModuleId(moduleRow.modelData.name)
                                    hoverEnabled: true

                                    background: Rectangle {
                                        radius: 7
                                        color: stateButton.down ? "#1B1027"
                                               : stateButton.hovered ? "#211331" : "#110B19"
                                        border.width: 2
                                        border.color: moduleRow.modelData.enabled
                                                      ? "#A855F7" : "#22D3EE"
                                    }
                                    contentItem: Text {
                                        text: stateButton.text
                                        color: moduleRow.modelData.enabled
                                               ? "#D8B4FE" : "#67E8F9"
                                        font.pixelSize: 11
                                        font.bold: true
                                        elide: Text.ElideRight
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    onClicked: {
                                        const verb = moduleRow.modelData.enabled
                                                     ? "disable" : "enable"
                                        root.runModuleAction(
                                            "/opt/neebles/client/bin/neebles modules "
                                            + verb + " "
                                            + root.shellArg(moduleRow.modelData.name),
                                            moduleRow.modelData.name
                                        )
                                    }
                                }
                            }

                            // Column 4 — Open exists only when module declares it.
                            Item {
                                Layout.preferredWidth: 74
                                Layout.fillHeight: true

                                PlasmaComponents3.Button {
                                    id: declaredOpenButton
                                    anchors.centerIn: parent
                                    width: 74
                                    height: 29
                                    visible: moduleRow.declaredOpen !== null
                                    text: moduleRow.declaredOpen
                                          ? moduleRow.declaredOpen.label : ""
                                    enabled: moduleRow.declaredOpen !== null
                                             && !root.modulePending(moduleRow.modelData.name)
                                             && moduleRow.declaredOpen.action_available === true
                                    hoverEnabled: true
                                    QQC2.ToolTip.visible: hovered && visible
                                    QQC2.ToolTip.text: text
                                    background: Rectangle {
                                        radius: 7
                                        color: declaredOpenButton.down ? "#0B1220"
                                               : declaredOpenButton.hovered ? "#172033" : "#10131A"
                                        border.width: 2
                                        border.color: "#22D3EE"
                                    }
                                    contentItem: Text {
                                        text: declaredOpenButton.text
                                        color: "#67E8F9"
                                        font.pixelSize: 11
                                        font.bold: true
                                        elide: Text.ElideRight
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    onClicked: {
                                        const item = moduleRow.declaredOpen
                                        if (item) {
                                            root.runModuleAction(
                                                root.surfaceActionCommand(
                                                    item.owner_module,
                                                    item.item_id,
                                                    item.data.action
                                                ),
                                                item.owner_module
                                            )
                                        }
                                    }
                                }
                            }

                            // Column 5 — reserved optional module-owned controls.
                            // Multiple declared controls stack vertically here;
                            // none are discarded and the ListView remains scrollable.
                            Item {
                                Layout.preferredWidth: 76
                                Layout.fillHeight: true

                                Column {
                                    anchors.centerIn: parent
                                    width: parent.width
                                    spacing: 4

                                    Repeater {
                                        model: moduleRow.remainingButtons

                                        delegate: PlasmaComponents3.Button {
                                            id: extraButton
                                            required property var modelData
                                            readonly property var item: modelData
                                            width: 76
                                            height: 26
                                            text: item.label
                                            enabled: !root.modulePending(item.owner_module)
                                                     && item.action_available === true
                                            hoverEnabled: true
                                            QQC2.ToolTip.visible: hovered
                                            QQC2.ToolTip.text: text
                                            background: Rectangle {
                                                radius: 6
                                                color: extraButton.hovered ? "#172033" : "#10131A"
                                                border.width: 1
                                                border.color: "#22D3EE"
                                            }
                                            contentItem: Text {
                                                text: extraButton.text
                                                font.pixelSize: 10
                                                color: "#67E8F9"
                                                elide: Text.ElideRight
                                                horizontalAlignment: Text.AlignHCenter
                                                verticalAlignment: Text.AlignVCenter
                                            }
                                            onClicked: {
                                                root.runModuleAction(
                                                    root.surfaceActionCommand(
                                                        item.owner_module,
                                                        item.item_id,
                                                        item.data.action
                                                    ),
                                                    item.owner_module
                                                )
                                            }
                                        }
                                    }

                                    Repeater {
                                        model: moduleRow.declaredSwitches

                                        delegate: Item {
                                            required property var modelData
                                            readonly property var item: modelData
                                            width: 76
                                            height: 26

                                            NeeblesSwitch {
                                                anchors.centerIn: parent
                                                checked: !!item.active
                                                checkable: false
                                                enabled: !root.modulePending(item.owner_module)
                                                         && item.requirements_met === true
                                                QQC2.ToolTip.visible: hovered
                                                QQC2.ToolTip.text: item.label
                                                onClicked: {
                                                    const data = item.data || ({})
                                                    const turnOn = !item.active
                                                    root.runModuleAction(
                                                        root.surfaceActionCommand(
                                                            item.owner_module,
                                                            item.item_id,
                                                            turnOn ? data.action_on : data.action_off
                                                        ),
                                                        item.owner_module
                                                    )
                                                }
                                            }
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

                Layout.preferredWidth: 175
                Layout.preferredHeight: 32
                Layout.alignment: Qt.AlignHCenter

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
