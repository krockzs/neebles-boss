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
     * Única instancia global del diálogo de desinstalación.
     */
    property string pendingUninstallModule: ""

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

                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 28
                                Layout.rightMargin: 0

                                visible: traySwitch.checked
                                spacing: 6

                                Repeater {
                                    model:
                                        typeof boss !== "undefined"
                                        ? boss.modules.filter(
                                            function(module) {
                                                return !!module.installed
                                                    && !!module.tray
                                            }
                                        )
                                        : []

                                    delegate: RowLayout {
                                        required property var modelData

                                        Layout.fillWidth: true
                                        Layout.leftMargin: 8
                                        spacing: 10

                                        Image {
                                            Layout.preferredWidth: 24
                                            Layout.preferredHeight: 24

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

                                        Label {
                                            Layout.preferredWidth: 160

                                            text: modelData.name

                                            color: "#67E8F9"
                                            font.pixelSize: 13
                                        }

                                        NeeblesSwitch {
                                            checked:
                                                typeof boss === "undefined"
                                                || boss.modules.length === 0
                                                || boss.hiddenTrayModules
                                                    .indexOf(
                                                        modelData.name
                                                    ) === -1

                                            checkable: false

                                            enabled:
                                                typeof boss === "undefined"
                                                || !boss.busy

                                            onClicked: {
                                                if (
                                                    typeof boss
                                                    === "undefined"
                                                    || boss.modules.length === 0
                                                )
                                                    return

                                                boss.setModuleVisibility(
                                                    "tray",
                                                    modelData.name,
                                                    !checked
                                                )
                                            }
                                        }
                                    }
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

                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 28
                                Layout.rightMargin: 0

                                visible: launcherSwitch.checked
                                spacing: 6

                                Repeater {
                                    model:
                                        typeof boss !== "undefined"
                                        ? boss.modules.filter(
                                            function(module) {
                                                return !!module.installed
                                                    && typeof module.launcher_action === "string"
                                                    && module.launcher_action.length > 0
                                            }
                                        )
                                        : []

                                    delegate: RowLayout {
                                        required property var modelData

                                        Layout.fillWidth: true
                                        Layout.leftMargin: 8
                                        spacing: 10

                                        Image {
                                            Layout.preferredWidth: 24
                                            Layout.preferredHeight: 24

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

                                        Label {
                                            Layout.preferredWidth: 160

                                            text: modelData.name

                                            color: "#67E8F9"
                                            font.pixelSize: 13
                                        }

                                        NeeblesSwitch {
                                            checked:
                                                typeof boss === "undefined"
                                                || boss.modules.length === 0
                                                || boss.hiddenLauncherModules
                                                    .indexOf(
                                                        modelData.name
                                                    ) === -1

                                            checkable: false

                                            enabled:
                                                typeof boss === "undefined"
                                                || !boss.busy

                                            onClicked: {
                                                if (
                                                    typeof boss
                                                    === "undefined"
                                                    || boss.modules.length === 0
                                                )
                                                    return

                                                boss.setModuleVisibility(
                                                    "launcher",
                                                    modelData.name,
                                                    !checked
                                                )
                                            }
                                        }
                                    }
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

                                readonly property var operationState:
                                    typeof boss !== "undefined"
                                    && boss.moduleOperations[
                                        modelData.name
                                    ]
                                    ? boss.moduleOperations[
                                        modelData.name
                                    ]
                                    : ({})

                                readonly property bool realOperationVisible:
                                    operationState.started === true

                                readonly property int realProgress:
                                    operationState.progress !== undefined
                                    ? operationState.progress
                                    : -1

                                readonly property bool effectiveInstalled:
                                    !!modelData.installed

                                readonly property bool effectiveEnabled:
                                    !!modelData.enabled

                                property bool detailsVisible: false

                                readonly property color moduleStateColor:
                                    !effectiveInstalled
                                    ? "#22D3EE"
                                    : effectiveEnabled
                                      ? "#A855F7"
                                      : "#52525B"

                                width:
                                    ListView.view.width

                                height:
                                    !realOperationVisible
                                    ? 116
                                    : detailsVisible
                                      ? 340
                                      : 205

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
                                                    moduleCard.detailsVisible =
                                                        false

                                                    if (
                                                        moduleCard.effectiveInstalled
                                                    ) {
                                                        const state =
                                                            boss.moduleLocalState(
                                                                modelData.name
                                                            )

                                                        if (state < 0)
                                                            return

                                                        if (state === 0) {
                                                            boss.uninstallModule(
                                                                modelData.name,
                                                                false
                                                            )
                                                        } else {
                                                            root.pendingUninstallModule =
                                                                modelData.name

                                                            uninstallSettingsDialog.open()
                                                        }
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
                                                    boss.setModuleEnabled(
                                                        modelData.name,
                                                        !modelData.enabled
                                                    )
                                                }
                                            }
                                        }

                                        ColumnLayout {
                                            spacing: 6

                                            Button {
                                                id: moduleOpenButton

                                                visible:
                                                    moduleCard.effectiveInstalled
                                                    && moduleCard.effectiveEnabled
                                                    && !modelData.running
                                                    && !modelData.update_available

                                                text:
                                                    root.t(
                                                        "common.open"
                                                    )

                                                enabled:
                                                    typeof boss !== "undefined"
                                                    && !boss.busy

                                                background: Rectangle {
                                                    radius: 7
                                                    color: "#10131A"

                                                    border.width: 2
                                                    border.color: "#22D3EE"
                                                }

                                                contentItem: Text {
                                                    text:
                                                        moduleOpenButton.text

                                                    color: "#67E8F9"

                                                    horizontalAlignment:
                                                        Text.AlignHCenter

                                                    verticalAlignment:
                                                        Text.AlignVCenter
                                                }

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
                                                    moduleCard.detailsVisible =
                                                        false

                                                    boss.updateModule(
                                                        modelData.name
                                                    )
                                                }
                                            }
                                        }
                                    }

                                    /*
                                     * PROCESO INDEPENDIENTE DEL MÓDULO.
                                     * Aparece sólo después de una operación.
                                     */
                                    ColumnLayout {
                                        Layout.fillWidth: true

                                        visible:
                                            moduleCard.realOperationVisible

                                        spacing: 5

                                        RowLayout {
                                            Layout.fillWidth: true

                                            /*
                                             * Barra morada.
                                             */
                                            ProgressBar {
                                                id: moduleProgress

                                                Layout.fillWidth: true
                                                Layout.preferredHeight: 20

                                                from: 0
                                                to: 1

                                                value:
                                                    moduleCard.realProgress >= 0
                                                    ? moduleCard.realProgress
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
                                                            parent.width
                                                            * moduleProgress
                                                                .position

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
                                                Layout.preferredWidth: 42

                                                text:
                                                    moduleCard.realProgress >= 0
                                                    ? moduleCard.realProgress
                                                      + "%"
                                                    : "..."

                                                color: "#D8B4FE"

                                                horizontalAlignment:
                                                    Text.AlignRight
                                            }

                                            Button {
                                                id: moduleCopyButton

                                                visible:
                                                    moduleCard.detailsVisible

                                                Layout.minimumWidth: 42
                                                Layout.preferredWidth: 42
                                                Layout.maximumWidth: 42

                                                Layout.minimumHeight: 42
                                                Layout.preferredHeight: 42
                                                Layout.maximumHeight: 42

                                                padding: 0
                                                hoverEnabled: true

                                                enabled:
                                                    moduleLogText.text.length > 0

                                                background: Item {
                                                }

                                                contentItem: Image {
                                                    anchors.fill: parent

                                                    source:
                                                        !moduleCopyButton.enabled
                                                        ? root.asset(
                                                            "copy-icon-disabled.png"
                                                        )
                                                        : moduleCopyButton.down
                                                          ? root.asset(
                                                              "copy-icon-pressed.png"
                                                          )
                                                          : moduleCopyButton.hovered
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
                                                    moduleLogText.selectAll()
                                                    moduleLogText.copy()
                                                    moduleLogText.deselect()
                                                }
                                            }

                                            /*
                                             * Misma flecha que el installer.
                                             */
                                            Button {
                                                id: moduleDetailsButton

                                                Layout.preferredWidth: 42
                                                Layout.preferredHeight: 42

                                                padding: 0
                                                hoverEnabled: true

                                                background: Item {
                                                }

                                                contentItem: Image {
                                                    anchors.fill: parent

                                                    source:
                                                        !moduleDetailsButton.enabled
                                                        ? root.asset(
                                                            "show_details_disabled.png"
                                                        )
                                                        : moduleDetailsButton.down
                                                          ? root.asset(
                                                              "show_details_pressed.png"
                                                          )
                                                          : moduleDetailsButton.hovered
                                                            ? root.asset(
                                                                "show_details_hover.png"
                                                            )
                                                            : root.asset(
                                                                "show_details_normal.png"
                                                            )

                                                    fillMode:
                                                        Image.PreserveAspectFit

                                                    smooth: true
                                                    mipmap: true

                                                    rotation:
                                                        moduleCard.detailsVisible
                                                        ? 180
                                                        : 0

                                                    Behavior on rotation {
                                                        NumberAnimation {
                                                            duration: 120
                                                        }
                                                    }
                                                }

                                                onClicked:
                                                    moduleCard.detailsVisible =
                                                        !moduleCard
                                                            .detailsVisible
                                            }
                                        }

                                        /*
                                         * Log de ESTE módulo.
                                         */
                                        Rectangle {
                                            Layout.fillWidth: true
                                            Layout.preferredHeight: 125

                                            visible:
                                                moduleCard.detailsVisible

                                            radius: 8
                                            color: "#050507"

                                            border.width: 2
                                            border.color: "#A855F7"

                                            Rectangle {
                                                anchors.fill: parent
                                                anchors.margins: -3

                                                z: -1

                                                radius: 10
                                                color: "transparent"

                                                border.width: 4
                                                border.color: "#4C1D95"

                                                opacity: 0.45
                                            }

                                            ScrollView {
                                                anchors.fill: parent
                                                anchors.margins: 8

                                                TextArea {
                                                    id: moduleLogText

                                                    readOnly: true
                                                    selectByMouse: true

                                                    wrapMode:
                                                        TextEdit.WrapAnywhere

                                                    color: "#D4D4D8"

                                                    selectionColor:
                                                        "#7C3AED"

                                                    selectedTextColor:
                                                        "#FFFFFF"

                                                    font.family:
                                                        "monospace"

                                                    font.pixelSize: 12

                                                    background: null

                                                    text:
                                                        moduleCard.operationState.log
                                                        !== undefined
                                                        ? moduleCard.operationState.log
                                                        : ""
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
    Dialog {
        id: uninstallSettingsDialog

        modal: true
        focus: true

        width:
            Math.min(
                500,
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

        onOpened:
            uninstallRemoveLocalState.checked = false

        background: Rectangle {
            radius: 14

            color: "#0C0C10"

            border.width: 2
            border.color: "#A855F7"

            Rectangle {
                anchors.fill: parent
                anchors.margins: -3

                z: -1

                radius: 17
                color: "transparent"

                border.width: 4
                border.color: "#4C1D95"

                opacity: 0.55
            }
        }

        contentItem: ColumnLayout {
            spacing: 18

            Label {
                Layout.fillWidth: true

                text:
                    root.t(
                        "modules.uninstall_title"
                    )

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
                    root.pendingUninstallModule

                color: "#A1A1AA"

                font.pixelSize: 13

                horizontalAlignment:
                    Text.AlignHCenter
            }

            CheckBox {
                id: uninstallRemoveLocalState

                Layout.fillWidth: true

                text:
                    root.t(
                        "modules.uninstall_remove_local_state"
                    )
            }

            RowLayout {
                Layout.fillWidth: true

                spacing: 10

                Button {
                    id: uninstallCancelButton

                    Layout.fillWidth: true
                    Layout.preferredHeight: 42

                    hoverEnabled: true

                    text:
                        root.t(
                            "common.cancel"
                        )

                    background: Rectangle {
                        radius: 8

                        color:
                            uninstallCancelButton.down
                            ? "#27272A"
                            : uninstallCancelButton.hovered
                              ? "#18181B"
                              : "#101014"

                        border.width: 1
                        border.color: "#52525B"
                    }

                    contentItem: Text {
                        text:
                            uninstallCancelButton.text

                        color: "#D4D4D8"

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked:
                        uninstallSettingsDialog.close()
                }

                Button {
                    id: uninstallConfirmButton

                    Layout.fillWidth: true
                    Layout.preferredHeight: 42

                    hoverEnabled: true

                    text:
                        root.t(
                            "modules.uninstall_confirm"
                        )

                    background: Rectangle {
                        radius: 8

                        color:
                            uninstallConfirmButton.down
                            ? "#6D28D9"
                            : uninstallConfirmButton.hovered
                              ? "#4C1D95"
                              : "#1B1027"

                        border.width: 2
                        border.color: "#A855F7"
                    }

                    contentItem: Text {
                        text:
                            uninstallConfirmButton.text

                        color: "#E9D5FF"

                        font.bold: true

                        horizontalAlignment:
                            Text.AlignHCenter

                        verticalAlignment:
                            Text.AlignVCenter
                    }

                    onClicked: {
                        const name =
                            root.pendingUninstallModule

                        const removeLocalState =
                            uninstallRemoveLocalState.checked

                        uninstallSettingsDialog.close()

                        if (
                            typeof boss !== "undefined"
                            && name.length > 0
                        ) {
                            boss.uninstallModule(
                                name,
                                removeLocalState
                            )
                        }
                    }
                }
            }
        }

        onClosed: {
            uninstallRemoveLocalState.checked = false
            root.pendingUninstallModule = ""
        }
    }

}
