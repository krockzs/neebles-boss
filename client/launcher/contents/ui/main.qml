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
    // Fresh canonical Surface Model availability; independent of delegate modelData caching.
    property var canonicalOpenActions: ({})
    property bool refreshBusy: false
    property bool refreshAgain: false
    property string commandError: ""
    property string bossVersion: "1.0.53"




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

    function canonicalOpenAvailable(moduleName, itemId) {
        const actions = canonicalOpenActions[moduleName]
        return !!actions && actions[itemId] === true
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
                // Rebuild from every authenticated, fresh Boss Surface Model read.
                // Open is permitted only while the runtime is closed and the
                // module-declared action is canonically available. This also
                // works for object-owned Surface Open without governor.open.
                const openActions = ({})
                for (const module of installed) {
                    if (!module || !module.name)
                        continue
                    const actions = ({})
                    for (const item of (module.surface_content || [])) {
                        if (item && item.surface === "launcher" && item.item_id
                            && item.data && item.data.action === "open") {
                            actions[item.item_id] =
                                module.runtime_state === "closed"
                                && item.action_available === true
                        }
                    }
                    openActions[module.name] = actions
                }
                canonicalOpenActions = openActions
                installedModules = installed
                applyModuleFilter()
            } catch (e) {
                // Fail closed for actions on read errors; retain only the last
                // visual model to avoid a disappearing Launcher panel.
                canonicalOpenActions = ({})
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

    // 1.0.48: Plasmoid identity remains stable; only child QML is replaceable.
    compactRepresentation: Loader {
        id: compactLoader
        property var bossRoot: root
        property var bossSettings: bossEvents
        source: "LauncherCompactSurface.qml?neeblesRevision=" + bossEvents.surfaceRevision
    }

    fullRepresentation: Loader {
        id: fullLoader
        property var bossRoot: root
        property var bossSettings: bossEvents
        source: "LauncherFullSurface.qml?neeblesRevision=" + bossEvents.surfaceRevision
    }

}
