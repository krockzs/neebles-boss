import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: root

    width: 980
    height: 640
    minimumWidth: 820
    minimumHeight: 520
    visible: true

    title: "N.E.E.B.L.E.S. Boss 1.0.0"
    color: "#09090B"

    property int page: 0

    property url sourceBrandingRoot:
        Qt.resolvedUrl("../../../client/assets/branding/")

    property url sourceFlagsRoot:
        Qt.resolvedUrl("../../../client/assets/flags/4x3/")

    property bool loadingConfig: true

    function asset(name) {
        if (typeof boss !== "undefined" && boss.assetUrl)
            return boss.assetUrl(name)

        return sourceBrandingRoot + name
    }

    function flag(name) {
        return sourceFlagsRoot + name
    }

    function shortLanguage(code) {
        if (code === "es_CL")
            return "ES"

        if (code === "en_US")
            return "EN"

        return code.substring(0, 2).toUpperCase()
    }

    function saveNow() {
        if (
            loadingConfig ||
            typeof boss === "undefined" ||
            languageBox.currentIndex < 0
        )
            return

        const selected =
            languageBox.model[languageBox.currentIndex]

        boss.saveConfig(
            selected.code,
            traySwitch.checked,
            launcherSwitch.checked,
            notificationSwitch.checked
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
                        : [
                            {
                                code: "es_CL",
                                name: "Español (Chile)",
                                flag: "cl.svg"
                            },
                            {
                                code: "en_US",
                                name: "English (United States)",
                                flag: "us.svg"
                            }
                        ]

                    Component.onCompleted:
                        syncLanguage()

                    onModelChanged:
                        syncLanguage()

                    function syncLanguage() {
                        const currentLanguage =
                            typeof boss !== "undefined"
                            ? boss.language
                            : "es_CL"

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

                    onActivated:
                        root.saveNow()
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

                Label {
                    text:
                        typeof boss !== "undefined"
                        ? boss.statusText
                        : ""

                    color:
                        text === "ERROR"
                        ? "#F87171"
                        : "#71717A"

                    visible:
                        text.length > 0
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
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 14

                        Switch {
                            id: traySwitch

                            text:
                                typeof boss !== "undefined"
                                ? boss.text(
                                    "config.tray"
                                )
                                : "Tray"

                            checked:
                                typeof boss !== "undefined"
                                ? boss.trayEnabled
                                : true

                            onToggled:
                                root.saveNow()
                        }

                        Switch {
                            id: launcherSwitch

                            text:
                                typeof boss !== "undefined"
                                ? boss.text(
                                    "config.launcher"
                                )
                                : "Launcher"

                            checked:
                                typeof boss !== "undefined"
                                ? boss.launcherEnabled
                                : true

                            onToggled:
                                root.saveNow()
                        }

                        Switch {
                            id: notificationSwitch

                            text:
                                typeof boss !== "undefined"
                                ? boss.text(
                                    "config.normal_notifications"
                                )
                                : "Notifications"

                            checked:
                                typeof boss !== "undefined"
                                ? boss.normalNotifications
                                : true

                            onToggled:
                                root.saveNow()
                        }

                        Item {
                            Layout.fillHeight: true
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
                                ? boss.text(
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
                                required property var modelData

                                width:
                                    ListView.view.width

                                height: 104
                                radius: 10

                                color: "#111116"
                                border.color: "#27272A"

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 14

                                    Image {
                                        Layout.preferredWidth:
                                            58

                                        Layout.preferredHeight:
                                            58

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

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 3

                                        Label {
                                            text:
                                                modelData.name
                                                || ""

                                            color: "#F5F5F5"
                                            font.pixelSize: 18
                                            font.bold: true
                                        }

                                        Label {
                                            text:
                                                modelData.version
                                                || ""

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

                                    ColumnLayout {
                                        spacing: 2

                                        Label {
                                            text:
                                                modelData.installed
                                                ? "Installed"
                                                : "Not installed"

                                            color:
                                                modelData.installed
                                                ? "#22D3EE"
                                                : "#A1A1AA"

                                            font.pixelSize: 12
                                        }

                                        Switch {
                                            checked:
                                                !!modelData.installed

                                            enabled:
                                                typeof boss === "undefined"
                                                || !boss.busy

                                            onToggled: {
                                                if (
                                                    typeof boss
                                                    === "undefined"
                                                )
                                                    return

                                                if (checked)
                                                    boss.installModule(
                                                        modelData.name
                                                    )
                                                else
                                                    boss.uninstallModule(
                                                        modelData.name
                                                    )
                                            }
                                        }
                                    }

                                    ColumnLayout {
                                        spacing: 2

                                        Label {
                                            text:
                                                modelData.installed
                                                ? (
                                                    modelData.enabled
                                                    ? "Active"
                                                    : "Not active"
                                                )
                                                : "Not active"

                                            color:
                                                modelData.installed
                                                && modelData.enabled
                                                ? "#A78BFA"
                                                : "#71717A"

                                            font.pixelSize: 12
                                        }

                                        Switch {
                                            checked:
                                                !!modelData.enabled

                                            enabled:
                                                !!modelData.installed
                                                && (
                                                    typeof boss
                                                    === "undefined"
                                                    || !boss.busy
                                                )

                                            onToggled: {
                                                if (
                                                    typeof boss
                                                    !== "undefined"
                                                )
                                                    boss.setModuleEnabled(
                                                        modelData.name,
                                                        checked
                                                    )
                                            }
                                        }
                                    }

                                    ColumnLayout {
                                        spacing: 6

                                        Button {
                                            visible:
                                                !!modelData.installed

                                            text:
                                                typeof boss !== "undefined"
                                                ? boss.text(
                                                    "common.open"
                                                )
                                                : "Open"

                                            enabled:
                                                !!modelData.enabled
                                                && (
                                                    typeof boss
                                                    === "undefined"
                                                    || !boss.busy
                                                )

                                            onClicked: {
                                                if (
                                                    typeof boss
                                                    !== "undefined"
                                                )
                                                    boss.openModule(
                                                        modelData.name
                                                    )
                                            }
                                        }

                                        Button {
                                            visible:
                                                !!modelData.installed

                                            text:
                                                typeof boss !== "undefined"
                                                ? boss.text(
                                                    "common.update"
                                                )
                                                : "Update"

                                            enabled:
                                                typeof boss === "undefined"
                                                || !boss.busy

                                            onClicked: {
                                                if (
                                                    typeof boss
                                                    !== "undefined"
                                                )
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
}
