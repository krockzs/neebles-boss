import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Controls as QQC2
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.plasma5support as Plasma5Support

Item {
    property var bossRoot: parent.bossRoot
    property var bossSettings: parent.bossSettings
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
                        mipmap: false
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter

                    text: "v" + bossRoot.bossVersion

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

                    model: bossRoot.modules

                    boundsBehavior: Flickable.StopAtBounds

                    delegate: Rectangle {
                        id: moduleRow

                        required property var modelData

                        // Keep the two mandatory identity columns. The three
                        // action slots do not create undeclared module actions.
                        readonly property var declaredButtons:
                            bossRoot.launcherActionButtons(modelData)
                        readonly property var declaredSwitches:
                            bossRoot.launcherStateSwitches(modelData)
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
                                          ? bossRoot.t("common.disable")
                                          : bossRoot.t("common.enable")
                                    enabled: !bossRoot.modulePending(moduleRow.modelData.name)
                                             && bossRoot.safeModuleId(moduleRow.modelData.name)
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
                                        bossRoot.runModuleAction(
                                            "/opt/neebles/client/bin/neebles modules "
                                            + verb + " "
                                            + bossRoot.shellArg(moduleRow.modelData.name),
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
                                             && !bossRoot.modulePending(moduleRow.modelData.name)
                                             && bossRoot.canonicalOpenAvailable(
                                                 moduleRow.modelData.name,
                                                 moduleRow.declaredOpen.item_id
                                             )
                                    hoverEnabled: true
                                    QQC2.ToolTip.visible: hovered && visible
                                    QQC2.ToolTip.text: text
                                    background: Rectangle {
                                        radius: 7
                                        color: !declaredOpenButton.enabled ? "#101014"
                                               : declaredOpenButton.down ? "#0B1220"
                                               : declaredOpenButton.hovered ? "#172033" : "#10131A"
                                        border.width: 2
                                        border.color: declaredOpenButton.enabled ? "#22D3EE" : "#52525B"
                                    }
                                    contentItem: Text {
                                        text: declaredOpenButton.text
                                        color: declaredOpenButton.enabled ? "#67E8F9" : "#71717A"
                                        font.pixelSize: 11
                                        font.bold: true
                                        elide: Text.ElideRight
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    onClicked: {
                                        const item = moduleRow.declaredOpen
                                        if (item) {
                                            bossRoot.runModuleAction(
                                                bossRoot.surfaceActionCommand(
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
                                            enabled: !bossRoot.modulePending(item.owner_module)
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
                                                bossRoot.runModuleAction(
                                                    bossRoot.surfaceActionCommand(
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
                                                enabled: !bossRoot.modulePending(item.owner_module)
                                                         && item.requirements_met === true
                                                QQC2.ToolTip.visible: hovered
                                                QQC2.ToolTip.text: item.label
                                                onClicked: {
                                                    const data = item.data || ({})
                                                    const turnOn = !item.active
                                                    bossRoot.runModuleAction(
                                                        bossRoot.surfaceActionCommand(
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
                        bossRoot.modules.length === 0

                    text:
                        bossRoot.t(
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
                    bossRoot.t(
                        "launcher.open_boss"
                    )

                enabled:
                    bossSettings.connected
                    && bossSettings.bossUiState === "closed"

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
                    bossRoot.exec(
                        "/opt/neebles/client/bin/neebles start",
                        function() {}
                    )
            }
        }
    }


// NEEBLES 1.0.48: update boundary for prior-version surface detector.
