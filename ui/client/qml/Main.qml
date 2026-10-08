import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: root

    component NeeblesSwitch: Switch {
        id: control

        implicitWidth: 42
        implicitHeight: 24

        indicator: Rectangle {
            implicitWidth: 36
            implicitHeight: 18
            radius: 9

            color:
                control.checked
                ? "#5B21B6"
                : "#27272A"

            border.width: 1
            border.color:
                control.checked
                ? "#A855F7"
                : "#3F3F46"

            Rectangle {
                width: 14
                height: 14
                radius: 7

                anchors.verticalCenter: parent.verticalCenter

                x:
                    control.checked
                    ? parent.width - width - 2
                    : 2

                color:
                    control.checked
                    ? "#E9D5FF"
                    : "#71717A"

                Behavior on x {
                    NumberAnimation {
                        duration: 120
                    }
                }
            }
        }

        contentItem: Text {
            text: control.text
            color: "#F5F5F5"
            font.pixelSize: 13

            leftPadding:
                control.indicator.width + 8

            verticalAlignment:
                Text.AlignVCenter
        }
    }

    width: 980
    height: 640
    minimumWidth: 820
    minimumHeight: 520
    visible: true

    title: root.t("app.title")
    color: "#09090B"

    property int page: 0

    property url sourceBrandingRoot:
        Qt.resolvedUrl("../../../client/assets/branding/")

    property url sourceFlagsRoot:
        Qt.resolvedUrl("../../../client/assets/flags/4x3/")

    property bool loadingConfig: true

    /*
     * Generic dependency warning state.
     *
     * Boss computes consequences.
     * QML only presents and confirms them.
     */
    property string pendingDependencyAction: ""
    property string pendingDependencyModule: ""
    property bool pendingDependencyAllowed: false
    property bool pendingDependencyHasLocalState: false
    property var pendingDependencyAffected: []
    property var pendingDependencyBlockers: []

    /*
     * Presentation cursor only.
     *
     * It prevents log/progress updates from reopening
     * a transaction dialog that the user deliberately
     * hid while execution continues.
     */
    property string lastPresentedTransactionId: ""


    function resetDependencyWarning() {
        pendingDependencyAction = ""
        pendingDependencyModule = ""
        pendingDependencyAllowed = false
        pendingDependencyHasLocalState = false
        pendingDependencyAffected = []
        pendingDependencyBlockers = []
    }

    function dependencyAdditionalAffected() {
        const result = []

        for (
            let index = 0;
            index < pendingDependencyAffected.length;
            ++index
        ) {
            const item =
                pendingDependencyAffected[index]

            if (item !== pendingDependencyModule)
                result.push(item)
        }

        return result
    }

    function dependencyPreflightFailed() {
        return (
            !pendingDependencyAllowed
            && pendingDependencyBlockers.length === 0
        )
    }

    function requestDependencyConfirmation(
        action,
        moduleName,
        hasLocalState
    ) {
        if (
            typeof boss === "undefined"
            || moduleName.length === 0
        ) {
            return
        }

        const preflight =
            boss.dependencyPreflight(
                action,
                moduleName
            )

        pendingDependencyAction =
            action

        pendingDependencyModule =
            moduleName

        pendingDependencyAllowed =
            !!preflight.allowed

        pendingDependencyHasLocalState =
            !!hasLocalState

        pendingDependencyAffected =
            preflight.affected
            ? preflight.affected
            : []

        pendingDependencyBlockers =
            preflight.blockers
            ? preflight.blockers
            : []

        /*
         * A simple enable/disable affecting only the
         * requested module does not need a warning.
         *
         * Cascades require explicit confirmation.
         * Uninstall always remains explicitly confirmed.
         */
        if (
            (
                action === "enable"
                || action === "disable"
            )
            && pendingDependencyAllowed
            && pendingDependencyAffected.length <= 1
        ) {
            boss.setModuleEnabled(
                moduleName,
                action === "enable"
            )

            resetDependencyWarning()
            return
        }

        dependencyWarningDialog.open()
        dependencyWarningDialog.forceActiveFocus()
    }

    function executeDependencyConfirmation() {
        if (
            typeof boss === "undefined"
            || !pendingDependencyAllowed
            || pendingDependencyModule.length === 0
        ) {
            return
        }

        const action =
            pendingDependencyAction

        const moduleName =
            pendingDependencyModule

        const removeLocalState =
            dependencyRemoveLocalState.checked

        /*
         * Explicit Accept is the only path that starts
         * the actual transaction.
         */
        dependencyWarningDialog.close()

        if (action === "enable") {
            boss.setModuleEnabled(
                moduleName,
                true
            )
        } else if (action === "disable") {
            boss.setModuleEnabled(
                moduleName,
                false
            )
        } else if (action === "uninstall") {
            boss.uninstallModule(
                moduleName,
                removeLocalState
            )
        }
    }

    function dependencyWarningTitle() {
        if (dependencyPreflightFailed()) {
            return root.t(
                "modules.dependency_warning_preflight_failed_title"
            )
        }

        if (
            pendingDependencyAction === "enable"
        ) {
            return root.t(
                "modules.dependency_warning_enable_title"
            )
        }

        if (
            pendingDependencyAction === "disable"
        ) {
            return root.t(
                "modules.dependency_warning_disable_title"
            )
        }

        if (
            pendingDependencyAction === "uninstall"
            && !pendingDependencyAllowed
        ) {
            return root.t(
                "modules.dependency_warning_uninstall_blocked_title"
            )
        }

        return root.t(
            "modules.dependency_warning_uninstall_title"
        )
    }

    function dependencyWarningBody() {
        if (dependencyPreflightFailed()) {
            return root.t(
                "modules.dependency_warning_preflight_failed_body"
            )
        }

        if (
            pendingDependencyAction === "enable"
        ) {
            return root.t(
                "modules.dependency_warning_enable_body"
            )
        }

        if (
            pendingDependencyAction === "disable"
        ) {
            return root.t(
                "modules.dependency_warning_disable_body"
            )
        }

        if (
            pendingDependencyAction === "uninstall"
            && !pendingDependencyAllowed
        ) {
            return root.t(
                "modules.dependency_warning_uninstall_blocked_body"
            )
        }

        return root.t(
            "modules.dependency_warning_uninstall_body"
        )
    }


    function asset(name) {
        if (typeof boss !== "undefined" && boss.assetUrl)
            return boss.assetUrl(name)

        return sourceBrandingRoot + name
    }

    function flag(name) {
        if (
            typeof boss !== "undefined"
            && boss.flagUrl
        )
            return boss.flagUrl(name)

        return sourceFlagsRoot + name
    }

    function t(key) {
        if (typeof boss === "undefined")
            return key

        const revision = boss.translationsRevision
        return boss.text(key)
    }

    function shortLanguage(code) {
        if (code === "es_CL")
            return "ES"

        if (code === "en_US")
            return "EN"

        return code.substring(0, 2).toUpperCase()
    }

    /*
     * Flatten canonical module SurfaceContent into presentation
     * controls for Boss Config.
     *
     * One row == one projection item.
     * A module may publish zero, one or many items on any surface.
     */
    function surfaceConfigItems(surfaceName) {
        var result = []

        if (
            typeof boss === "undefined"
            || !boss.modules
            || boss.modules.length === undefined
        )
            return result

        for (
            var moduleIndex = 0;
            moduleIndex < boss.modules.length;
            ++moduleIndex
        ) {
            var module = boss.modules[moduleIndex]

            if (
                !module
                || !module.installed
                || !module.surface_content
                || module.surface_content.length === undefined
            )
                continue

            for (
                var itemIndex = 0;
                itemIndex < module.surface_content.length;
                ++itemIndex
            ) {
                var item =
                    module.surface_content[itemIndex]

                if (
                    !item
                    || item.surface !== surfaceName
                    || !item.item_id
                )
                    continue

                result.push({
                    "moduleName": module.name,
                    "moduleIcon": module.icon
                        ? module.icon
                        : "",
                    "item": item
                })
            }
        }

        return result
    }

    function surfaceConfigModules(surfaceName) {
        var result = []

        if (
            typeof boss === "undefined"
            || !boss.modules
            || boss.modules.length === undefined
        )
            return result

        for (
            var moduleIndex = 0;
            moduleIndex < boss.modules.length;
            ++moduleIndex
        ) {
            var module = boss.modules[moduleIndex]

            if (
                !module
                || !module.installed
                || !module.surface_content
                || module.surface_content.length === undefined
            )
                continue

            var declaresSurface = false

            for (
                var itemIndex = 0;
                itemIndex < module.surface_content.length;
                ++itemIndex
            ) {
                var item =
                    module.surface_content[itemIndex]

                if (
                    item
                    && item.surface === surfaceName
                ) {
                    declaresSurface = true
                    break
                }
            }

            if (!declaresSurface)
                continue

            var visibility =
                module.surface_visibility
                ? module.surface_visibility
                : ({})

            result.push({
                "moduleName": module.name,
                "moduleIcon": module.icon
                    ? module.icon
                    : "",
                "visible":
                    visibility[surfaceName]
                    === true
            })
        }

        return result
    }

    function surfaceConfigLabel(entry) {
        if (
            !entry
            || !entry.item
        )
            return ""

        var data =
            entry.item.data
            ? entry.item.data
            : ({})

        if (
            typeof entry.item.label === "string"
            && entry.item.label.length > 0
        )
            return entry.item.label

        if (
            typeof data.label === "string"
            && data.label.length > 0
        )
            return data.label

        if (
            typeof data.label_key === "string"
            && data.label_key.length > 0
        )
            return data.label_key

        return entry.moduleName
            + " · "
            + entry.item.item_id
    }


    function surfaceFeatureItems() {
        var entries =
            root.surfaceConfigItems("ui")

        var result = []

        for (
            var index = 0;
            index < entries.length;
            ++index
        ) {
            var entry = entries[index]

            if (
                !entry
                || !entry.item
                || entry.item.visible !== true
            )
                continue

            var item = entry.item

            var data =
                item.data
                ? item.data
                : ({})

            if (
                data.control === "button"
                && data.action
                && data.label_key
            ) {
                result.push(entry)
                continue
            }

            if (
                data.control === "switch"
                && item.object_id
                && item.active !== undefined
                && item.active !== null
                && data.label_key
                && data.action_on
                && data.action_off
                && data.transition_on
                && data.transition_off
            ) {
                result.push(entry)
            }
        }

        return result
    }

    function saveConfigValue(key, value) {
        if (
            loadingConfig
            || typeof boss === "undefined"
        )
            return

        boss.saveConfigValue(
            key,
            String(value)
        )
    }

    Component.onCompleted: loadingConfig = false

    Rectangle {
        anchors.fill: parent
        color: "#09090B"

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 30
            spacing: 14

            /*
             * HEADER
             */
            RowLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 144

                spacing: 18

                Image {
                    Layout.preferredWidth: 176
                    Layout.preferredHeight: 144

                    source:
                        root.asset(
                            "neebles-boss-wall-transparent.png"
                        )

                    fillMode: Image.PreserveAspectFit
                    smooth: true
                    mipmap: true
                }

                Item {
                    Layout.fillWidth: true
                }

                Button {
                    id: criticalUpdateButton

                    visible:
                        typeof boss !== "undefined"
                        && boss.bossUpdateAvailable

                    Layout.preferredWidth: 190
                    Layout.preferredHeight: 42

                    Layout.alignment:
                        Qt.AlignTop | Qt.AlignHCenter

                    hoverEnabled: true

                    text:
                        typeof boss !== "undefined"
                        ? "Critical Update "
                          + boss.bossRemoteVersion
                        : "Critical Update"

                    background: Rectangle {
                        radius: 10

                        color:
                            criticalUpdateButton.down
                            ? "#3F0A0A"
                            : criticalUpdateButton.hovered
                              ? "#2B1010"
                              : "#180B0B"

                        border.width: 2
                        border.color: "#EF4444"

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -3

                            z: -1
                            radius: 13
                            color: "transparent"

                            border.width: 4
                            border.color: "#DC2626"

                            opacity: 0.55
                        }
                    }

                    contentItem: Text {
                        text:
                            criticalUpdateButton.text

                        color: "#FCA5A5"

                        font.pixelSize: 13
                        font.bold: true

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    enabled:
                        typeof boss !== "undefined"
                        && boss.bossUpdateAvailable
                        && !boss.busy

                    onClicked:
                        boss.installBossUpdate()
                }

                Button {
                    id: telemetryButton

                    visible:
                        !telemetryDialog.opened

                    Layout.preferredWidth: 180
                    Layout.preferredHeight: 42

                    Layout.alignment:
                        Qt.AlignTop | Qt.AlignHCenter

                    hoverEnabled: true

                    text:
                        root.t("telemetry.settings")

                    background: Rectangle {
                        radius: 10

                        color:
                            telemetryButton.down
                            ? "#24102F"
                            : telemetryButton.hovered
                              ? "#1B1027"
                              : "#101014"

                        border.width: 2
                        border.color: "#EF4444"

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -3

                            z: -1

                            radius: 13
                            color: "transparent"

                            border.width: 4
                            border.color: "#DC2626"

                            opacity: 0.55
                        }
                    }

                    contentItem: Text {
                        text:
                            telemetryButton.text

                        color: "#E9D5FF"

                        font.pixelSize: 13
                        font.bold: true

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        telemetryDialog.open()
                }

                /*
                 * LANGUAGE SELECTOR
                 *
                 * Asset original:
                 * 1035 × 196
                 *
                 * 210 × 40 mantiene prácticamente
                 * su proporción original.
                 */
                ComboBox {
                    id: languageBox

                    /*
                     * language_selector.png
                     *
                     * Canvas original: 2048 × 682
                     * Franja visual útil: y ~= 125..535
                     * Divisor vertical: x ~= 1701
                     */
                    Layout.preferredWidth: 210
                    Layout.preferredHeight: 45

                    padding: 0

                    Layout.alignment:
                        Qt.AlignTop | Qt.AlignRight

                    /*
                     * Asset reducido real:
                     * 520 × 111
                     *
                     * Divisor medido:
                     * x = 435
                     */
                    property real sourceWidth: 520
                    property real sourceDividerX: 435

                    /*
                     * Posición del divisor dentro
                     * del selector ya renderizado.
                     */
                    readonly property real dividerX:
                        width
                        * sourceDividerX
                        / sourceWidth

                    model:
                        typeof boss !== "undefined"
                        ? boss.languages
                        : []

                    Component.onCompleted:
                        syncLanguage()

                    onModelChanged:
                        syncLanguage()

                    function syncLanguage() {
                        const currentLanguage =
                            typeof boss !== "undefined"
                            ? boss.language
                            : ""

                        for (let i = 0; i < count; ++i) {
                            if (model[i].code === currentLanguage) {
                                currentIndex = i
                                return
                            }
                        }
                    }

                    /*
                     * FLAG + ES/EN
                     *
                     * Todo el grupo queda centrado
                     * matemáticamente dentro de la
                     * sección izquierda del asset.
                     */
                    contentItem: Item {
                        anchors.fill: parent

                        Row {
                            id: selectedLanguageRow

                            spacing: 8

                            anchors.verticalCenter:
                                parent.verticalCenter

                            /*
                             * Centro de la sección izquierda:
                             *
                             * dividerX / 2
                             *
                             * Luego desplazamos la mitad
                             * del ancho real del grupo.
                             */
                            x: 14

                            Image {
                                width: 25
                                height: 19

                                anchors.verticalCenter:
                                    parent.verticalCenter

                                source:
                                    languageBox.currentIndex >= 0
                                    ? root.flag(
                                        languageBox.model[
                                            languageBox.currentIndex
                                        ].flag
                                    )
                                    : ""

                                fillMode:
                                    Image.PreserveAspectFit

                                smooth: true
                            }

                            Text {
                                anchors.verticalCenter:
                                    parent.verticalCenter

                                text:
                                    languageBox.currentIndex >= 0
                                    ? root.shortLanguage(
                                        languageBox.model[
                                            languageBox.currentIndex
                                        ].code
                                    )
                                    : ""

                                color: "#F5F5F5"

                                font.pixelSize: 14
                                font.bold: true
                            }
                        }
                    }

                    /*
                     * FLECHA REAL
                     *
                     * Centrada exclusivamente dentro
                     * del compartimento derecho.
                     */
                    indicator: Item {
                        width:
                            languageBox.width
                            - languageBox.dividerX

                        height:
                            languageBox.height

                        x:
                            languageBox.dividerX

                        y: 0

                        Canvas {
                            anchors.fill: parent

                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.reset()

                                const cx = width / 2 - 4
                                const cy = height / 2 + 1

                                ctx.beginPath()

                                ctx.moveTo(
                                    cx - 4,
                                    cy - 2
                                )

                                ctx.lineTo(
                                    cx,
                                    cy + 2
                                )

                                ctx.lineTo(
                                    cx + 4,
                                    cy - 2
                                )

                                ctx.strokeStyle =
                                    languageBox.pressed
                                    ? "#FFFFFF"
                                    : "#D8B4FE"

                                ctx.lineWidth = 2
                                ctx.lineCap = "round"
                                ctx.lineJoin = "round"

                                ctx.stroke()
                            }
                        }
                    }

                    /*
                     * Lista desplegable.
                     */
                    delegate: ItemDelegate {
                        required property var modelData

                        width:
                            languageBox.width

                        contentItem: Row {
                            spacing: 8

                            Image {
                                width: 28
                                height: 21

                                anchors.verticalCenter:
                                    parent.verticalCenter

                                source:
                                    root.flag(
                                        modelData.flag
                                    )

                                fillMode:
                                    Image.PreserveAspectFit

                                smooth: true
                            }

                            Text {
                                anchors.verticalCenter:
                                    parent.verticalCenter

                                text:
                                    modelData.name

                                color: "#F5F5F5"
                                font.pixelSize: 14
                            }
                        }

                        background: Rectangle {
                            color:
                                parent.highlighted
                                ? "#27272A"
                                : "#111116"
                        }
                    }

                    /*
                     * CARCASA
                     *
                     * Recortamos la transparencia
                     * vertical sobrante del PNG.
                     *
                     * Así el divisor dibujado y el
                     * divisor lógico usan exactamente
                     * la misma proporción horizontal.
                     */
                    background: Image {
                        anchors.fill: parent

                        source:
                            root.asset(
                                "language_selector.png"
                            )

                        fillMode:
                            Image.Stretch

                        smooth: true
                        mipmap: true
                    }

                    onActivated: {
                        if (
                            languageBox.currentIndex < 0
                            || typeof boss === "undefined"
                        )
                            return

                        const selected =
                            languageBox.model[
                                languageBox.currentIndex
                            ]

                        root.saveConfigValue(
                            "language",
                            selected.code
                        )
                    }
                }
            }

            /*
             * MAIN NAVIGATION
             */
            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Button {
                    id: configButton

                    Layout.preferredWidth: 66
                    Layout.preferredHeight: 66

                    onClicked:
                        root.page = 0

                    background: Rectangle {
                        radius: 10

                        color:
                            root.page === 0
                            ? "#17121F"
                            : "transparent"

                        border.width:
                            root.page === 0
                            ? 1
                            : 0

                        border.color:
                            "#7C3AED"
                    }

                    contentItem: Image {
                        source:
                            root.asset(
                                "config_icon.png"
                            )

                        fillMode:
                            Image.PreserveAspectFit

                        anchors.margins: 4

                        smooth: true
                        mipmap: true
                    }
                }

                Button {
                    id: modulesButton

                    Layout.preferredWidth: 66
                    Layout.preferredHeight: 66

                    onClicked:
                        root.page = 1

                    background: Rectangle {
                        radius: 10

                        color:
                            root.page === 1
                            ? "#101A1E"
                            : "transparent"

                        border.width:
                            root.page === 1
                            ? 1
                            : 0

                        border.color:
                            "#22D3EE"
                    }

                    contentItem: Image {
                        source:
                            root.asset(
                                "modules_icon.png"
                            )

                        fillMode:
                            Image.PreserveAspectFit

                        anchors.margins: 4

                        smooth: true
                        mipmap: true
                    }
                }

                Item {
                    Layout.fillWidth: true
                }

            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1

                color: "#27272A"
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true

                currentIndex:
                    root.page

                /*
                 * CONFIG
                 */
                Item {
                    Flickable {
                        anchors.fill: parent

                        contentWidth: width
                        contentHeight: configColumn.implicitHeight
                        clip: true

                        ColumnLayout {
                            id: configColumn

                            width: parent.width
                            spacing: 14

                            NeeblesSwitch {
                                id: traySwitch

                                text:
                                    root.t("config.tray")

                                checked:
                                    typeof boss !== "undefined"
                                    ? boss.trayEnabled
                                    : true

                                onToggled:
                                    root.saveConfigValue(
                                        "tray_enabled",
                                        checked
                                    )
                            }

                            ListView {
                                id: trayConfigList

                                Layout.fillWidth: true
                                Layout.leftMargin: 28

                                Layout.preferredHeight:
                                    visible
                                    ? Math.min(
                                        144,
                                        Math.max(
                                            36,
                                            contentHeight
                                        )
                                    )
                                    : 0

                                visible:
                                    traySwitch.checked
                                    && count > 0

                                clip: true
                                spacing: 6

                                model:
                                    typeof boss !== "undefined"
                                    ? root.surfaceConfigModules("tray")
                                    : []

                                boundsBehavior:
                                    Flickable.StopAtBounds

                                delegate: RowLayout {
                                    required property var modelData

                                    width:
                                        ListView.view.width

                                    height: 34
                                    spacing: 10

                                    Image {
                                        Layout.preferredWidth: 24
                                        Layout.preferredHeight: 24

                                        source:
                                            modelData.moduleIcon
                                            && modelData.moduleIcon.length > 0
                                            ? modelData.moduleIcon
                                            : root.asset(
                                                "modules_icon.png"
                                            )

                                        fillMode:
                                            Image.PreserveAspectFit

                                        smooth: true
                                        mipmap: true
                                    }

                                    Label {
                                        Layout.fillWidth: true

                                        text:
                                            modelData.moduleName

                                        color: "#67E8F9"
                                        font.pixelSize: 13

                                        elide:
                                            Text.ElideRight
                                    }

                                    NeeblesSwitch {
                                        checked:
                                            !!modelData.visible

                                        checkable: false

                                        enabled:
                                            typeof boss === "undefined"
                                            || !boss.busy

                                        onClicked: {
                                            if (
                                                typeof boss
                                                === "undefined"
                                            )
                                                return

                                            boss.setSurfaceModuleVisibility(
                                                "tray",
                                                modelData.moduleName,
                                                !checked
                                            )
                                        }
                                    }
                                }

                                ScrollBar.vertical: ScrollBar {
                                    policy:
                                        trayConfigList.contentHeight
                                        > trayConfigList.height
                                        ? ScrollBar.AsNeeded
                                        : ScrollBar.AlwaysOff
                                }
                            }
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 1
                                color: "#18181B"
                            }

                            NeeblesSwitch {
                                id: launcherSwitch

                                text:
                                    root.t("config.launcher")

                                checked:
                                    typeof boss !== "undefined"
                                    ? boss.launcherEnabled
                                    : true

                                onToggled:
                                    root.saveConfigValue(
                                        "launcher_enabled",
                                        checked
                                    )
                            }

                            ListView {
                                id: launcherConfigList

                                Layout.fillWidth: true
                                Layout.leftMargin: 28

                                Layout.preferredHeight:
                                    visible
                                    ? Math.min(
                                        144,
                                        Math.max(
                                            36,
                                            contentHeight
                                        )
                                    )
                                    : 0

                                visible:
                                    launcherSwitch.checked
                                    && count > 0

                                clip: true
                                spacing: 6

                                model:
                                    typeof boss !== "undefined"
                                    ? root.surfaceConfigModules("launcher")
                                    : []

                                boundsBehavior:
                                    Flickable.StopAtBounds

                                delegate: RowLayout {
                                    required property var modelData

                                    width:
                                        ListView.view.width

                                    height: 34
                                    spacing: 10

                                    Image {
                                        Layout.preferredWidth: 24
                                        Layout.preferredHeight: 24

                                        source:
                                            modelData.moduleIcon
                                            && modelData.moduleIcon.length > 0
                                            ? modelData.moduleIcon
                                            : root.asset(
                                                "modules_icon.png"
                                            )

                                        fillMode:
                                            Image.PreserveAspectFit

                                        smooth: true
                                        mipmap: true
                                    }

                                    Label {
                                        Layout.fillWidth: true

                                        text:
                                            modelData.moduleName

                                        color: "#67E8F9"
                                        font.pixelSize: 13

                                        elide:
                                            Text.ElideRight
                                    }

                                    NeeblesSwitch {
                                        checked:
                                            !!modelData.visible

                                        checkable: false

                                        enabled:
                                            typeof boss === "undefined"
                                            || !boss.busy

                                        onClicked: {
                                            if (
                                                typeof boss
                                                === "undefined"
                                            )
                                                return

                                            boss.setSurfaceModuleVisibility(
                                                "launcher",
                                                modelData.moduleName,
                                                !checked
                                            )
                                        }
                                    }
                                }

                                ScrollBar.vertical: ScrollBar {
                                    policy:
                                        launcherConfigList.contentHeight
                                        > launcherConfigList.height
                                        ? ScrollBar.AsNeeded
                                        : ScrollBar.AlwaysOff
                                }
                            }
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 1
                                color: "#18181B"
                            }

                            Label {
                                Layout.fillWidth: true

                                text:
                                    root.t(
                                        "config.features"
                                    )

                                color: "#D8B4FE"
                                font.pixelSize: 14
                                font.bold: true
                            }

                            ListView {
                                id: featureConfigList

                                Layout.fillWidth: true
                                Layout.leftMargin: 28

                                Layout.preferredHeight:
                                    visible
                                    ? Math.min(
                                        176,
                                        Math.max(
                                            44,
                                            contentHeight
                                        )
                                    )
                                    : 0

                                visible:
                                    count > 0

                                clip: true
                                spacing: 6

                                model:
                                    typeof boss !== "undefined"
                                    ? root.surfaceFeatureItems()
                                    : []

                                boundsBehavior:
                                    Flickable.StopAtBounds

                                delegate: RowLayout {
                                    required property var modelData

                                    property var surfaceItem:
                                        modelData.item

                                    property var surfaceData:
                                        surfaceItem
                                        && surfaceItem.data
                                        ? surfaceItem.data
                                        : ({})

                                    width:
                                        ListView.view.width

                                    height: 40
                                    spacing: 10

                                    opacity:
                                        surfaceItem
                                        && surfaceItem.requirements_met
                                        === true
                                        ? 1.0
                                        : 0.55

                                    Image {
                                        Layout.preferredWidth: 26
                                        Layout.preferredHeight: 26

                                        source:
                                            modelData.moduleIcon
                                            && modelData.moduleIcon.length > 0
                                            ? modelData.moduleIcon
                                            : root.asset(
                                                "modules_icon.png"
                                            )

                                        fillMode:
                                            Image.PreserveAspectFit

                                        smooth: true
                                        mipmap: true
                                    }

                                    Button {
                                        id: featureActionButton

                                        visible:
                                            surfaceData.control
                                            === "button"

                                        Layout.fillWidth: false
                                        Layout.preferredWidth:
                                            Math.max(
                                                96,
                                                implicitWidth
                                            )
                                        Layout.preferredHeight: 34
                                        Layout.alignment:
                                            Qt.AlignLeft
                                            | Qt.AlignVCenter

                                        text:
                                            root.surfaceConfigLabel(
                                                modelData
                                            )

                                        enabled:
                                            typeof boss
                                            !== "undefined"
                                            && !boss.busy
                                            && surfaceItem.requirements_met
                                            === true

                                        hoverEnabled: true

                                        background: Rectangle {
                                            radius: 7

                                            color:
                                                featureActionButton.down
                                                ? "#0B1220"
                                                : featureActionButton.hovered
                                                  ? "#172033"
                                                  : "#10131A"

                                            border.width:
                                                featureActionButton.activeFocus
                                                ? 3
                                                : 2

                                            border.color:
                                                featureActionButton.enabled
                                                ? "#22D3EE"
                                                : "#52525B"
                                        }

                                        contentItem: Text {
                                            text:
                                                featureActionButton.text

                                            color:
                                                featureActionButton.enabled
                                                ? "#67E8F9"
                                                : "#64748B"

                                            horizontalAlignment:
                                                Text.AlignHCenter

                                            verticalAlignment:
                                                Text.AlignVCenter
                                        }

                                        onClicked: {
                                            boss.requestSurfaceAction(
                                                surfaceItem.owner_module,
                                                surfaceItem.item_id,
                                                surfaceData.action
                                            )
                                        }
                                    }

                                    Label {
                                        visible:
                                            surfaceData.control
                                            === "switch"

                                        Layout.fillWidth: true

                                        text:
                                            root.surfaceConfigLabel(
                                                modelData
                                            )

                                        color: "#A78BFA"
                                        font.pixelSize: 12

                                        elide:
                                            Text.ElideRight
                                    }

                                    NeeblesSwitch {
                                        visible:
                                            surfaceData.control
                                            === "switch"

                                        checked:
                                            !!surfaceItem.active

                                        checkable: false

                                        enabled:
                                            typeof boss
                                            !== "undefined"
                                            && !boss.busy
                                            && surfaceItem.requirements_met
                                            === true

                                        onClicked: {
                                            const turnOn =
                                                !surfaceItem.active

                                            boss.requestSurfaceAction(
                                                surfaceItem.owner_module,
                                                surfaceItem.item_id,
                                                turnOn
                                                ? surfaceData.action_on
                                                : surfaceData.action_off
                                            )
                                        }
                                    }
                                }

                                ScrollBar.vertical: ScrollBar {
                                    policy:
                                        featureConfigList.contentHeight
                                        > featureConfigList.height
                                        ? ScrollBar.AsNeeded
                                        : ScrollBar.AlwaysOff
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 1
                                color: "#18181B"
                            }
                            NeeblesSwitch {
                                id: notificationSwitch

                                text:
                                    root.t(
                                        "config.normal_notifications"
                                    )

                                checked:
                                    typeof boss !== "undefined"
                                    ? boss.normalNotifications
                                    : true

                                onToggled:
                                    root.saveConfigValue(
                                        "normal_notifications",
                                        checked
                                    )
                            }

                            Item {
                                Layout.fillHeight: true
                            }
                        }
                    }
                }

                /*
                 * MODULES
                 */
                Item {
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 12

                        RowLayout {
                            Layout.fillWidth: true

                            Item {
                                Layout.fillWidth: true
                            }

                            /*
                             * UPDATE / REFRESH
                             *
                             * Asset:
                             * 300 × 270
                             *
                             * Visual:
                             * 60 × 54
                             *
                             * Misma proporción.
                             */
                            Button {
                                id: updateButton

                                Layout.preferredWidth: 60
                                Layout.preferredHeight: 54

                                hoverEnabled: true
                                padding: 0

                                enabled:
                                    typeof boss === "undefined"
                                    || !boss.busy

                                background: Item {
                                }

                                contentItem: Image {
                                    anchors.fill: parent

                                    source:
                                        !updateButton.enabled
                                        ? root.asset(
                                            "update_disabled.png"
                                        )
                                        : updateButton.down
                                        ? root.asset(
                                            "update_pressed.png"
                                        )
                                        : updateButton.hovered
                                        ? root.asset(
                                            "update_hover.png"
                                        )
                                        : root.asset(
                                            "update_normal.png"
                                        )

                                    fillMode:
                                        Image.PreserveAspectFit

                                    smooth: true
                                    mipmap: true
                                }

                                onClicked: {
                                    if (
                                        typeof boss
                                        !== "undefined"
                                    )
                                        boss.reload()
                                }
                            }
                        }

                        Label {
                            visible:
                                typeof boss !== "undefined"
                                && boss.modules.length === 0

                            text:
                                typeof boss !== "undefined"
                                ? root.t(
                                    "modules.empty"
                                )
                                : ""

                            color: "#71717A"
                        }

                        ListView {
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            spacing: 10
                            clip: true

                            model:
                                typeof boss !== "undefined"
                                ? boss.modules
                                : []

                            delegate: Rectangle {
                                id: moduleCard

                                required property var modelData

                                readonly property bool effectiveInstalled:
                                    !!modelData.installed

                                readonly property bool effectiveEnabled:
                                    !!modelData.enabled

                                readonly property color moduleStateColor:
                                    !effectiveInstalled
                                    ? "#22D3EE"
                                    : effectiveEnabled
                                      ? "#A855F7"
                                      : "#52525B"

                                width:
                                    ListView.view.width

                                height: 116

                                radius: 12
                                color: "#0C0C10"

                                border.width: 2
                                border.color: "#22D3EE"

                                Behavior on height {
                                    NumberAnimation {
                                        duration: 150
                                    }
                                }

                                /*
                                 * Glow cian exterior de la tarjeta.
                                 */
                                Rectangle {
                                    anchors.fill: parent
                                    anchors.margins: -2

                                    z: -1

                                    radius: 14
                                    color: "transparent"

                                    border.width: 3
                                    border.color: "#164E63"

                                    opacity: 0.65
                                }



                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 14

                                    spacing: 10

                                    /*
                                     * CABECERA DEL MÓDULO
                                     */
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 82

                                        spacing: 14

                                        /*
                                         * ICONO CON ESTADO
                                         */
                                        Item {
                                            Layout.minimumWidth: 72
                                            Layout.preferredWidth: 72
                                            Layout.maximumWidth: 72

                                            Layout.minimumHeight: 72
                                            Layout.preferredHeight: 72
                                            Layout.maximumHeight: 72

                                            Rectangle {
                                                anchors.fill: parent

                                                radius: 14
                                                color: "transparent"

                                                border.width: 5

                                                border.color:
                                                    moduleCard.moduleStateColor

                                                opacity:
                                                    moduleCard.effectiveInstalled
                                                    && !moduleCard.effectiveEnabled
                                                    ? 0.45
                                                    : 0.28
                                            }

                                            Rectangle {
                                                anchors.fill: parent
                                                anchors.margins: 4

                                                radius: 11
                                                color: "#09090D"

                                                border.width: 2

                                                border.color:
                                                    moduleCard.moduleStateColor
                                            }

                                            Image {
                                                anchors.fill: parent
                                                anchors.margins: 10

                                                source:
                                                    modelData.icon
                                                    && modelData.icon.length > 0
                                                    ? modelData.icon
                                                    : root.asset(
                                                        "modules_icon.png"
                                                    )

                                                fillMode:
                                                    Image.PreserveAspectFit

                                                smooth: true
                                                mipmap: true
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 3

                                            Label {
                                                text:
                                                    modelData.name || ""

                                                color: "#67E8F9"

                                                font.pixelSize: 18
                                                font.bold: true
                                            }

                                            Label {
                                                text:
                                                    modelData.version || ""

                                                color: "#A78BFA"
                                                font.pixelSize: 13
                                            }

                                            Label {
                                                visible:
                                                    !!modelData.description

                                                text:
                                                    modelData.description
                                                    || ""

                                                color: "#A1A1AA"

                                                elide:
                                                    Text.ElideRight

                                                Layout.fillWidth: true
                                            }
                                        }

                                        /*
                                         * INSTALAR / DESINSTALAR
                                         */
                                        ColumnLayout {
                                            spacing: 2

                                            Label {
                                                text:
                                                    moduleCard.effectiveInstalled
                                                    ? root.t(
                                                        "modules.installed"
                                                    )
                                                    : root.t(
                                                        "common.install"
                                                    )

                                                color: "#67E8F9"
                                                font.pixelSize: 12
                                            }

                                            NeeblesSwitch {
                                                checked:
                                                    moduleCard.effectiveInstalled

                                                checkable: false

                                                enabled:
                                                    typeof boss !== "undefined"
                                                    && !boss.busy
                                                    && !modelData.running

                                                onClicked: {
if (
                                                        moduleCard.effectiveInstalled
                                                    ) {
                                                        const state =
                                                            boss.moduleLocalState(
                                                                modelData.name
                                                            )

                                                        if (state < 0)
                                                            return

                                                        root.requestDependencyConfirmation(
                                                            "uninstall",
                                                            modelData.name,
                                                            state === 1
                                                        )
                                                    } else {
                                                        boss.installModule(
                                                            modelData.name
                                                        )
                                                    }
                                                }
                                            }
                                        }

                                        /*
                                         * ENABLE / DISABLE
                                         */
                                        ColumnLayout {
                                            visible:
                                                moduleCard.effectiveInstalled

                                            spacing: 2

                                            Label {
                                                text:
                                                    moduleCard.effectiveEnabled
                                                    ? root.t(
                                                        "modules.active"
                                                    )
                                                    : root.t(
                                                        "modules.inactive"
                                                    )

                                                color:
                                                    moduleCard.effectiveEnabled
                                                    ? "#A78BFA"
                                                    : "#71717A"

                                                font.pixelSize: 12
                                            }

                                            NeeblesSwitch {
                                                checked:
                                                    moduleCard.effectiveEnabled

                                                checkable: false

                                                enabled:
                                                    typeof boss !== "undefined"
                                                    && !boss.busy

                                                onClicked: {
                                                    root.requestDependencyConfirmation(
                                                        modelData.enabled
                                                        ? "disable"
                                                        : "enable",
                                                        modelData.name,
                                                        false
                                                    )
                                                }
                                            }
                                        }

                                        Button {
                                            id: moduleOpenButton

                                            visible:
                                                moduleCard.effectiveInstalled

                                            Layout.preferredWidth: 82
                                            Layout.preferredHeight: 34

                                            text:
                                                root.t("common.open")

                                            enabled:
                                                typeof boss !== "undefined"
                                                && !boss.busy
                                                && moduleCard.effectiveEnabled
                                                && modelData.open_available
                                                === true

                                            onClicked: {
                                                boss.openModule(
                                                    modelData.name
                                                )
                                            }
                                        }

                                            Button {
                                                id: moduleUpdateButton

                                                visible:
                                                    moduleCard.effectiveInstalled
                                                    && !!modelData.update_available

                                                text:
                                                    root.t(
                                                        "common.update"
                                                    )

                                                enabled:
                                                    typeof boss !== "undefined"
                                                    && !boss.busy

                                                background: Rectangle {
                                                    radius: 7
                                                    color: "#1B1027"

                                                    border.width: 2
                                                    border.color: "#A855F7"
                                                }

                                                contentItem: Text {
                                                    text:
                                                        moduleUpdateButton.text

                                                    color: "#E9D5FF"

                                                    horizontalAlignment:
                                                        Text.AlignHCenter

                                                    verticalAlignment:
                                                        Text.AlignVCenter
                                                }

                                                onClicked: {
boss.updateModule(
                                                        modelData.name
                                                    )
                                                }
                                            }
                                    }


                                }
                            }
                        }
                    }
                }
            }
        }
    }


    /*
     * ==========================================================
     * TELEMETRY SETTINGS
     * ==========================================================
     */
    Dialog {
        id: telemetryDialog

        modal: true
        focus: true

        width:
            Math.min(
                520,
                root.width - 80
            )

        x:
            Math.round(
                (root.width - width) / 2
            )

        y:
            Math.round(
                (root.height - height) / 2
            )

        padding: 24

        closePolicy:
            Popup.CloseOnEscape
            | Popup.CloseOnPressOutside

        background: Rectangle {
            radius: 14

            color: "#0C0C10"

            border.width: 2
            border.color: "#EF4444"

            Rectangle {
                anchors.fill: parent
                anchors.margins: -3

                z: -1

                radius: 17
                color: "transparent"

                border.width: 4
                border.color: "#991B1B"

                opacity: 0.55
            }
        }

        contentItem: ColumnLayout {
            spacing: 18

            Label {
                Layout.fillWidth: true

                text:
                    root.t("telemetry.settings")

                color: "#E9D5FF"

                font.pixelSize: 20
                font.bold: true

                horizontalAlignment:
                    Text.AlignHCenter
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1

                color: "#4C1D95"
            }

            Label {
                Layout.fillWidth: true

                text:
                    root.t("telemetry.description")

                color: "#A1A1AA"

                font.pixelSize: 13

                wrapMode:
                    Text.WordWrap
            }

            NeeblesSwitch {
                id: telemetrySwitch

                Layout.fillWidth: true

                text:
                    root.t("telemetry.enable")

                checked:
                    typeof boss !== "undefined"
                    ? boss.telemetryEnabled
                    : false

                onToggled:
                    root.saveConfigValue(
                        "telemetry_enabled",
                        checked
                    )
            }

            Button {
                id: telemetryInformationButton

                Layout.fillWidth: true
                Layout.preferredHeight: 34

                hoverEnabled: true

                background: Item {
                }

                contentItem: Text {
                    text:
                        root.t(
                            "telemetry.more_information"
                        )

                    color:
                        telemetryInformationButton.hovered
                        ? "#67E8F9"
                        : "#22D3EE"

                    font.pixelSize: 13
                    font.underline: true

                    horizontalAlignment:
                        Text.AlignHCenter

                    verticalAlignment:
                        Text.AlignVCenter
                }

                onClicked:
                    Qt.openUrlExternally(
                        "https://github.com/krockzs/neebles-boss/blob/main/TELEMETRY.md"
                    )
            }

            Button {
                id: telemetryExitButton

                Layout.fillWidth: true
                Layout.preferredHeight: 42

                hoverEnabled: true

                text:
                    root.t("telemetry.exit")

                background: Rectangle {
                    radius: 8

                    color:
                        telemetryExitButton.down
                        ? "#6D28D9"
                        : telemetryExitButton.hovered
                          ? "#4C1D95"
                          : "#1B1027"

                    border.width: 2
                    border.color: "#A855F7"
                }

                contentItem: Text {
                    text:
                        telemetryExitButton.text

                    color: "#E9D5FF"

                    font.bold: true

                    horizontalAlignment:
                        Text.AlignHCenter

                    verticalAlignment:
                        Text.AlignVCenter
                }

                onClicked:
                    telemetryDialog.close()
            }
        }
    }


    /*
     * ==========================================================
     * MODAL GLOBAL DE DESINSTALACIÓN
     *
     * Una sola instancia para todos los módulos.
     * ==========================================================
     */
    Connections {
        target:
            typeof boss !== "undefined"
            ? boss
            : null

        function onTransactionOperationChanged() {
            if (
                typeof boss === "undefined"
                || !boss.transactionOperation
            ) {
                return
            }

            const transaction =
                boss.transactionOperation

            const id =
                transaction.id !== undefined
                ? String(transaction.id)
                : ""

            if (
                transaction.started === true
                && id.length > 0
                && id !== root.lastPresentedTransactionId
            ) {
                root.lastPresentedTransactionId =
                    id

                transactionProcessDialog.open()
                transactionProcessDialog.forceActiveFocus()
            }
        }
    }

    Dialog {
        id: transactionProcessDialog

        parent: Overlay.overlay

        modal: true
        focus: true

        width:
            Math.min(
                760,
                root.width - 64
            )

        height:
            Math.min(
                560,
                root.height - 64
            )

        x:
            Math.round(
                (parent.width - width) / 2
            )

        y:
            Math.round(
                (parent.height - height) / 2
            )

        closePolicy:
            Popup.CloseOnEscape
            | Popup.CloseOnPressOutside

        padding: 22

        /*
         * Closing this presentation NEVER cancels the
         * underlying transaction.
         *
         * Real execution cancellation must later use
         * Lifecycle CancellationToken/control path.
         */
        background: Rectangle {
            radius: 14
            color: "#100D14"

            border.width: 2
            border.color: "#7C3AED"

            Rectangle {
                anchors.fill: parent
                anchors.margins: -5

                z: -1

                radius: 18
                color: "transparent"

                border.width: 2
                border.color: "#A855F7"

                opacity: 0.38
            }
        }

        contentItem: ColumnLayout {
            spacing: 14

            readonly property var transaction:
                typeof boss !== "undefined"
                && boss.transactionOperation
                ? boss.transactionOperation
                : ({})

            readonly property int progressValue:
                transaction.progress !== undefined
                ? transaction.progress
                : -1

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 3

                    Label {
                        Layout.fillWidth: true

                        text:
                            root.t(
                                "modules.transaction.title"
                            )

                        color: "#F5D0FE"

                        font.pixelSize: 20
                        font.bold: true
                    }

                    Label {
                        Layout.fillWidth: true

                        text:
                            transactionProcessDialog.contentItem.transaction.operation
                            !== undefined
                            && transactionProcessDialog.contentItem.transaction.module
                            !== undefined
                            ? transactionProcessDialog.contentItem.transaction.operation
                              + " · "
                              + transactionProcessDialog.contentItem.transaction.module
                            : ""

                        color: "#C4B5FD"

                        font.pixelSize: 13

                        elide:
                            Text.ElideRight
                    }
                }

                Button {
                    id: transactionCloseButton

                    Layout.preferredWidth: 36
                    Layout.preferredHeight: 36

                    padding: 0
                    hoverEnabled: true

                    text: "×"

                    background: Rectangle {
                        radius: 8

                        color:
                            transactionCloseButton.down
                            ? "#24102F"
                            : transactionCloseButton.hovered
                              ? "#1B1027"
                              : "transparent"

                        border.width:
                            transactionCloseButton.activeFocus
                            ? 1
                            : 0

                        border.color: "#A78BFA"
                    }

                    contentItem: Text {
                        text:
                            transactionCloseButton.text

                        color:
                            transactionCloseButton.hovered
                            ? "#FFFFFF"
                            : "#D8B4FE"

                        font.pixelSize: 22

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        transactionProcessDialog.close()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                ProgressBar {
                    id: transactionProgress

                    Layout.fillWidth: true
                    Layout.preferredHeight: 20

                    from: 0
                    to: 1

                    indeterminate:
                        transactionProcessDialog.contentItem.progressValue < 0
                        && transactionProcessDialog.contentItem.transaction.running === true

                    value:
                        transactionProcessDialog.contentItem.progressValue >= 0
                        ? transactionProcessDialog.contentItem.progressValue
                          / 100.0
                        : 0

                    background: Rectangle {
                        implicitHeight: 10
                        radius: 5

                        color: "#18111F"

                        border.width: 1
                        border.color: "#6D28D9"
                    }

                    contentItem: Item {
                        implicitHeight: 10

                        Rectangle {
                            width:
                                transactionProgress.visualPosition
                                * parent.width

                            height:
                                parent.height

                            radius: 5

                            color: "#A855F7"

                            border.width: 1
                            border.color: "#D8B4FE"
                        }
                    }
                }

                Label {
                    Layout.preferredWidth: 54

                    text:
                        transactionProcessDialog.contentItem.progressValue >= 0
                        ? transactionProcessDialog.contentItem.progressValue
                          + "%"
                        : "..."

                    color: "#D8B4FE"

                    horizontalAlignment:
                        Text.AlignRight
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true

                radius: 10

                color: "#09090B"

                border.width: 1
                border.color: "#3F3F46"

                ScrollView {
                    anchors.fill: parent
                    anchors.margins: 8

                    TextArea {
                        id: transactionLogText

                        width:
                            parent.width

                        readOnly: true
                        selectByMouse: true

                        wrapMode:
                            TextEdit.NoWrap

                        color: "#E4E4E7"
                        selectionColor: "#7C3AED"
                        selectedTextColor: "#FFFFFF"

                        font.family: "monospace"
                        font.pixelSize: 12

                        background: null

                        text:
                            transactionProcessDialog.contentItem.transaction.log
                            !== undefined
                            ? transactionProcessDialog.contentItem.transaction.log
                            : ""
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                Label {
                    Layout.fillWidth: true

                    text:
                        transactionProcessDialog.contentItem.transaction.running === true
                        ? root.t(
                            "modules.transaction.running"
                        )
                        : transactionProcessDialog.contentItem.transaction.success === true
                          ? root.t(
                              "modules.transaction.success"
                          )
                          : root.t(
                              "modules.transaction.failed"
                          )

                    color:
                        transactionProcessDialog.contentItem.transaction.running === true
                        ? "#C4B5FD"
                        : transactionProcessDialog.contentItem.transaction.success === true
                          ? "#86EFAC"
                          : "#FCA5A5"

                    font.bold: true
                }

                Button {
                    id: transactionCopyButton

                    Layout.minimumWidth: 42
                    Layout.preferredWidth: 42
                    Layout.maximumWidth: 42

                    Layout.minimumHeight: 42
                    Layout.preferredHeight: 42
                    Layout.maximumHeight: 42

                    padding: 0
                    hoverEnabled: true

                    enabled:
                        transactionLogText.text.length > 0

                    background: Item {
                    }

                    contentItem: Image {
                        anchors.fill: parent

                        source:
                            !transactionCopyButton.enabled
                            ? root.asset(
                                "copy-icon-disabled.png"
                            )
                            : transactionCopyButton.down
                              ? root.asset(
                                  "copy-icon-pressed.png"
                              )
                              : transactionCopyButton.hovered
                                ? root.asset(
                                    "copy-icon-hover.png"
                                )
                                : root.asset(
                                    "copy-icon-normal.png"
                                )

                        fillMode:
                            Image.PreserveAspectFit

                        smooth: true
                        mipmap: true
                    }

                    onClicked: {
                        transactionLogText.selectAll()
                        transactionLogText.copy()
                        transactionLogText.deselect()
                    }
                }

                Button {
                    id: transactionHideButton

                    Layout.preferredWidth: 110
                    Layout.preferredHeight: 42

                    hoverEnabled: true

                    text:
                        root.t(
                            "modules.transaction.close"
                        )

                    background: Rectangle {
                        radius: 8

                        color:
                            transactionHideButton.down
                            ? "#27272A"
                            : transactionHideButton.hovered
                              ? "#3F3F46"
                              : "#18181B"

                        border.width:
                            transactionHideButton.activeFocus
                            ? 2
                            : 1

                        border.color:
                            transactionHideButton.activeFocus
                            ? "#A78BFA"
                            : "#52525B"
                    }

                    contentItem: Text {
                        text:
                            transactionHideButton.text

                        color: "#E4E4E7"

                        font.bold: true

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        transactionProcessDialog.close()
                }
            }
        }

        onOpened:
            transactionHideButton.forceActiveFocus()
    }

    Dialog {
        id: dependencyWarningDialog

        parent: Overlay.overlay

        modal: true
        focus: true

        width:
            Math.min(
                560,
                root.width - 64
            )

        x:
            Math.round(
                (parent.width - width) / 2
            )

        y:
            Math.round(
                (parent.height - height) / 2
            )

        closePolicy:
            Popup.CloseOnEscape
            | Popup.CloseOnPressOutside

        padding: 24

        background: Rectangle {
            radius: 14
            color: "#120A12"

            border.width: 2
            border.color: "#FF334F"

            Rectangle {
                anchors.fill: parent
                anchors.margins: -5

                radius: 18
                color: "transparent"

                border.width: 2
                border.color: "#B91C3B"

                opacity: 0.48
                z: -1
            }

            Rectangle {
                anchors.fill: parent
                anchors.margins: -10

                radius: 22
                color: "transparent"

                border.width: 1
                border.color: "#7F1D2D"

                opacity: 0.28
                z: -2
            }
        }

        contentItem: ColumnLayout {
            spacing: 16

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                Label {
                    Layout.fillWidth: true

                    text:
                        root.dependencyWarningTitle()

                    color: "#F5D0FE"

                    font.pixelSize: 20
                    font.bold: true

                    wrapMode:
                        Text.WordWrap
                }

                Button {
                    id: dependencyWarningCloseButton

                    Layout.preferredWidth: 34
                    Layout.preferredHeight: 34

                    hoverEnabled: true
                    flat: true

                    text: "×"

                    background: Rectangle {
                        radius: 8

                        color:
                            dependencyWarningCloseButton.down
                            ? "#3F1720"
                            : dependencyWarningCloseButton.hovered
                              ? "#2A1118"
                              : "transparent"

                        border.width:
                            dependencyWarningCloseButton.activeFocus
                            ? 1
                            : 0

                        border.color: "#A78BFA"
                    }

                    contentItem: Text {
                        text:
                            dependencyWarningCloseButton.text

                        color:
                            dependencyWarningCloseButton.hovered
                            ? "#FFFFFF"
                            : "#D8B4FE"

                        font.pixelSize: 22

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        dependencyWarningDialog.close()
                }
            }

            Label {
                Layout.fillWidth: true

                text:
                    root.dependencyWarningBody()

                color: "#D4D4D8"
                font.pixelSize: 14

                wrapMode:
                    Text.WordWrap
            }

            ColumnLayout {
                Layout.fillWidth: true

                visible:
                    root.pendingDependencyAllowed
                    && root.dependencyAdditionalAffected().length > 0

                spacing: 8

                Label {
                    Layout.fillWidth: true

                    text:
                        root.t(
                            "modules.dependency_warning_affected"
                        )

                    color: "#F0ABFC"
                    font.bold: true
                }

                Repeater {
                    model:
                        root.dependencyAdditionalAffected()

                    delegate: Label {
                        required property var modelData

                        Layout.fillWidth: true
                        Layout.leftMargin: 12

                        text:
                            "• " + modelData

                        color: "#E4E4E7"

                        wrapMode:
                            Text.WordWrap
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true

                visible:
                    !root.pendingDependencyAllowed
                    && root.pendingDependencyBlockers.length > 0

                spacing: 8

                Label {
                    Layout.fillWidth: true

                    text:
                        root.t(
                            "modules.dependency_warning_blockers"
                        )

                    color: "#FB7185"
                    font.bold: true
                }

                Repeater {
                    model:
                        root.pendingDependencyBlockers

                    delegate: Label {
                        required property var modelData

                        Layout.fillWidth: true
                        Layout.leftMargin: 12

                        text:
                            "• " + modelData

                        color: "#F4F4F5"

                        wrapMode:
                            Text.WordWrap
                    }
                }
            }

            NeeblesSwitch {
                id: dependencyRemoveLocalState

                Layout.fillWidth: true

                visible:
                    root.pendingDependencyAction === "uninstall"
                    && root.pendingDependencyAllowed
                    && root.pendingDependencyHasLocalState

                checked: false

                text:
                    root.t(
                        "modules.dependency_warning_remove_local_state"
                    )
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                Button {
                    id: dependencyWarningCancelButton

                    Layout.fillWidth: true
                    Layout.preferredHeight: 42

                    hoverEnabled: true

                    text:
                        root.pendingDependencyAllowed
                        ? root.t(
                            "common.cancel"
                        )
                        : root.t(
                            "modules.dependency_warning_understood"
                        )

                    background: Rectangle {
                        radius: 8

                        color:
                            dependencyWarningCancelButton.down
                            ? "#27272A"
                            : dependencyWarningCancelButton.hovered
                              ? "#3F3F46"
                              : "#18181B"

                        border.width:
                            dependencyWarningCancelButton.activeFocus
                            ? 2
                            : 1

                        border.color:
                            dependencyWarningCancelButton.activeFocus
                            ? "#A78BFA"
                            : "#52525B"
                    }

                    contentItem: Text {
                        text:
                            dependencyWarningCancelButton.text

                        color: "#E4E4E7"
                        font.bold: true

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        dependencyWarningDialog.close()
                }

                Button {
                    id: dependencyWarningAcceptButton

                    Layout.fillWidth: true
                    Layout.preferredHeight: 42

                    visible:
                        root.pendingDependencyAllowed

                    hoverEnabled: true

                    text:
                        root.t(
                            "modules.dependency_warning_accept"
                        )

                    background: Rectangle {
                        radius: 8

                        color:
                            dependencyWarningAcceptButton.down
                            ? "#991B1B"
                            : dependencyWarningAcceptButton.hovered
                              ? "#EF4444"
                              : "#B91C1C"

                        border.width:
                            dependencyWarningAcceptButton.activeFocus
                            ? 3
                            : 1

                        border.color:
                            dependencyWarningAcceptButton.activeFocus
                            ? "#FCA5A5"
                            : dependencyWarningAcceptButton.hovered
                              ? "#F87171"
                              : "#EF4444"
                    }

                    contentItem: Text {
                        text:
                            dependencyWarningAcceptButton.text

                        color: "#FFF1F2"
                        font.bold: true

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        root.executeDependencyConfirmation()
                }
            }
        }

        onOpened:
            dependencyWarningCancelButton.forceActiveFocus()

        onClosed: {
            dependencyRemoveLocalState.checked = false
            root.resetDependencyWarning()
        }
    }

}
