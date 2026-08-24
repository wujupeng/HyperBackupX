import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import HyperBackupX.Tray 1.0

ApplicationWindow {
    visible: false
    width: 400
    height: 600
    title: "HyperBackup X"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    color: "#1e1e2e"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: "HyperBackup X"
                font.pixelSize: 20
                font.bold: true
                color: "#cdd6f4"
            }

            Item { Layout.fillWidth: true }

            Text {
                text: TrayModel.agentVersion
                font.pixelSize: 12
                color: "#6c7086"
            }
        }

        StatusView {
            Layout.fillWidth: true
            Layout.preferredHeight: 120
            statusText: TrayModel.agentStatus
            hasAlert: TrayModel.hasAlert
        }

        Text {
            text: "Last Backup: " + TrayModel.lastBackupTime
            font.pixelSize: 12
            color: "#a6adc8"
        }

        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: "#45475a"
        }

        Text {
            text: "Jobs"
            font.pixelSize: 16
            font.bold: true
            color: "#cdd6f4"
        }

        JobListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Button {
                text: "Refresh"
                onClicked: TrayIPC.refreshStatus()
            }

            Button {
                text: "Trigger Backup"
                onClicked: TrayIPC.triggerBackup("")
            }

            Item { Layout.fillWidth: true }

            Button {
                text: "Close"
                onClicked: window.hide()
            }
        }
    }
}
