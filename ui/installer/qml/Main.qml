import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Effects

ApplicationWindow {
    id: window

    width: 820
    height: detailsVisible ? 620 : 420

    minimumWidth: 760
    minimumHeight: 400

    visible: true

    title: "N.E.E.B.L.E.S. Installer " + Qt.application.version
    color: "#09090B"

    property bool detailsVisible: false

    property url sourceBrandingRoot:
        Qt.resolvedUrl(
            "../../../client/assets/branding/"
        )

    /*
     * progress_bar.png
     *
     * Tamaño original:
     * 2172 × 724
     *
     * Usamos solamente la franja visual
     * que contiene barra + glow.
     *
     * Crop:
     * y = 215
     * h = 290
     *
     * Canal interior medido
     * en coordenadas DEL PNG ORIGINAL:
     *
     * x1 ≈ 82
     * x2 ≈ 2090
     * y1 ≈ 316
     * y2 ≈ 373
     */
    readonly property real progressSourceWidth: 2172
    readonly property real progressCropY: 215
    readonly property real progressCropHeight: 290

    readonly property real channelSourceLeft: 82
    readonly property real channelSourceRight: 2090
    readonly property real channelSourceTop: 316
    readonly property real channelSourceBottom: 373

    function asset(name) {
        if (
            typeof brandingRootUrl !== "undefined"
            && brandingRootUrl.toString().length > 0
        )
            return brandingRootUrl.toString() + name

        return sourceBrandingRoot + name
    }

    function progressValue() {
        if (typeof installer === "undefined")
            return 0

        return Math.max(
            0,
            Math.min(
                100,
                installer.progress
            )
        )
    }

    Component.onCompleted: {
        if (
            typeof installer !== "undefined"
            && installer.startInstallation
        )
            installer.startInstallation()
    }

    Connections {
        target:
            typeof installer !== "undefined"
            ? installer
            : null

        function onLogLine(line) {
            logArea.text +=
                (
                    logArea.text.length > 0
                    ? "\n"
                    : ""
                )
                + line

            logArea.cursorPosition =
                logArea.length
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#09090B"

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 30

            spacing: 18

            /*
             * HEADER
             */
            RowLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 144

                Image {
                    Layout.preferredWidth: 176
                    Layout.preferredHeight: 144

                    source:
                        window.asset(
                            "neebles-boss-wall-transparent.png"
                        )

                    fillMode:
                        Image.PreserveAspectFit

                    smooth: true
                    mipmap: true
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            /*
             * STATUS TEXT
             */
            Label {
                Layout.fillWidth: true

                text:
                    typeof installer !== "undefined"
                    ? installer.status
                    : ""

                color: "#F5F5F5"

                font.pixelSize: 18

                wrapMode:
                    Text.WordWrap
            }

            /*
             * CUSTOM N.E.E.B.L.E.S.
             * PROGRESS BAR
             */
            Item {
                id: progressShell

                Layout.fillWidth: true

                /*
                 * Mantiene la proporción REAL
                 * de la región recortada,
                 * no del PNG completo.
                 */
                Layout.preferredHeight:
                    width > 0
                    ? width
                      * window.progressCropHeight
                      / window.progressSourceWidth
                    : 100

                /*
                 * CANAL PROGRAMADO
                 *
                 * Se calcula usando exactamente
                 * las mismas coordenadas que
                 * progress_bar.png.
                 */
                Item {
                    id: progressChannel

                    x:
                        progressShell.width
                        * window.channelSourceLeft
                        / window.progressSourceWidth

                    width:
                        progressShell.width
                        * (
                            window.channelSourceRight
                            - window.channelSourceLeft
                        )
                        / window.progressSourceWidth

                    y:
                        progressShell.height
                        * (
                            window.channelSourceTop
                            - window.progressCropY
                        )
                        / window.progressCropHeight

                    height:
                        progressShell.height
                        * (
                            window.channelSourceBottom
                            - window.channelSourceTop
                        )
                        / window.progressCropHeight

                    /*
                     * PROGRESO REAL.
                     *
                     * No hay imagen de progreso.
                     * Esto se pinta dinámicamente.
                     */
                    Canvas {
                        id: progressFill

                        anchors.fill: parent

                        property real fraction:
                            window.progressValue() / 100.0

                        onFractionChanged:
                            requestPaint()

                        onWidthChanged:
                            requestPaint()

                        onHeightChanged:
                            requestPaint()

                        onPaint: {
                            const ctx =
                                getContext("2d")

                            ctx.reset()

                            const w =
                                width

                            const h =
                                height

                            const p =
                                Math.max(
                                    0,
                                    Math.min(
                                        1,
                                        fraction
                                    )
                                )

                            const fillWidth =
                                w * p

                            if (
                                fillWidth <= 0
                                || h <= 0
                            )
                                return

                            const r =
                                Math.min(
                                    h / 2,
                                    fillWidth
                                )

                            ctx.beginPath()

                            /*
                             * Comienzo redondeado.
                             */
                            ctx.moveTo(
                                r,
                                0
                            )

                            /*
                             * Frente SUPERIOR recto
                             * hasta el corte.
                             */
                            ctx.lineTo(
                                fillWidth,
                                0
                            )

                            /*
                             * Sólo en 100%
                             * dibujamos la curva
                             * derecha.
                             */
                            if (p >= 0.9999) {
                                ctx.quadraticCurveTo(
                                    w,
                                    0,
                                    w,
                                    h / 2
                                )

                                ctx.quadraticCurveTo(
                                    w,
                                    h,
                                    w - h / 2,
                                    h
                                )
                            } else {
                                ctx.lineTo(
                                    fillWidth,
                                    h
                                )
                            }

                            /*
                             * Parte inferior.
                             */
                            ctx.lineTo(
                                r,
                                h
                            )

                            /*
                             * Curva izquierda.
                             */
                            ctx.quadraticCurveTo(
                                0,
                                h,
                                0,
                                h - r
                            )

                            ctx.lineTo(
                                0,
                                r
                            )

                            ctx.quadraticCurveTo(
                                0,
                                0,
                                r,
                                0
                            )

                            ctx.closePath()

                            /*
                             * Glow interno del
                             * progreso.
                             */
                            ctx.shadowColor =
                                "#A855F7"

                            ctx.shadowBlur =
                                10

                            const gradient =
                                ctx.createLinearGradient(
                                    0,
                                    0,
                                    0,
                                    h
                                )

                            gradient.addColorStop(
                                0.0,
                                "#D8B4FE"
                            )

                            gradient.addColorStop(
                                0.32,
                                "#A855F7"
                            )

                            gradient.addColorStop(
                                1.0,
                                "#6D28D9"
                            )

                            ctx.fillStyle =
                                gradient

                            ctx.fill()
                        }
                    }
                }

                /*
                 * La carcasa queda ENCIMA
                 * del progreso.
                 *
                 * El interior del PNG es
                 * semitransparente, por eso
                 * deja ver el progreso.
                 */
                Image {
                    anchors.fill: parent

                    source:
                        window.asset(
                            "progress_bar.png"
                        )

                    sourceClipRect:
                        Qt.rect(
                            0,
                            window.progressCropY,
                            window.progressSourceWidth,
                            window.progressCropHeight
                        )

                    fillMode:
                        Image.Stretch

                    smooth: true
                    mipmap: true
                }

                /*
                 * Porcentaje dinámico.
                 *
                 * Ya no existe debajo
                 * de la barra.
                 */
                Text {
                    anchors.centerIn:
                        parent

                    text:
                        Math.round(
                            window.progressValue()
                        )
                        + "%"

                    color: "#F5F5F5"

                    font.pixelSize: 14
                    font.bold: true

                    style:
                        Text.Outline

                    styleColor:
                        "#50000000"
                }
            }

            /*
             * SHOW DETAILS
             */
            RowLayout {
                Layout.fillWidth: true

                Item {
                    Layout.fillWidth: true
                }

                Button {
                    id: detailsButton

                    Layout.preferredWidth: 58
                    Layout.preferredHeight: 58

                    padding: 0
                    hoverEnabled: true

                    background: Item {
                    }

                    contentItem: Image {
                        anchors.fill: parent

                        /*
                         * Down = pressed
                         * Hover = hover
                         * Disabled = disabled
                         * Else = normal
                         */
                        source:
                            !detailsButton.enabled
                            ? window.asset(
                                "show_details_disabled.png"
                            )
                            : detailsButton.down
                            ? window.asset(
                                "show_details_pressed.png"
                            )
                            : detailsButton.hovered
                            ? window.asset(
                                "show_details_hover.png"
                            )
                            : window.asset(
                                "show_details_normal.png"
                            )

                        fillMode:
                            Image.PreserveAspectFit

                        smooth: true
                        mipmap: true

                        /*
                         * Down arrow:
                         * show details.
                         *
                         * Up arrow:
                         * hide details.
                         */
                        rotation:
                            window.detailsVisible
                            ? 180
                            : 0

                        Behavior on rotation {
                            NumberAnimation {
                                duration: 120
                            }
                        }
                    }

                    onClicked:
                        window.detailsVisible =
                            !window.detailsVisible
                }
            }

            /*
             * DETAILS / LOG
             */
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                visible:
                    window.detailsVisible

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 2

                    radius: 8
                    color: "#050507"

                    border.width: 2
                    border.color: "#A855F7"

                    layer.enabled: true
                    layer.effect: MultiEffect {
                        shadowEnabled: true
                        shadowColor: "#A855F7"
                        shadowOpacity: 0.55
                        shadowBlur: 1.0
                        shadowHorizontalOffset: 0
                        shadowVerticalOffset: 0
                    }

                    clip: true

                    ScrollView {
                    anchors.fill: parent
                    anchors.margins: 10

                    TextArea {
                        id: logArea

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

                        text: ""
                    }
                }
            }

            }

            Label {
                Layout.fillWidth: true

                visible:
                    typeof installer !== "undefined"
                    && installer.finished

                text:
                    typeof installer !== "undefined"
                    && installer.success
                    ? "N.E.E.B.L.E.S. is ready."
                    : "Installation did not complete. Review the details above."

                color:
                    typeof installer !== "undefined"
                    && installer.success
                    ? "#22D3EE"
                    : "#F87171"

                font.pixelSize: 14
            }
        }
    }
}
