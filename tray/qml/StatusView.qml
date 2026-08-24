import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    property string statusText: "idle"
    property bool hasAlert: false

    readonly property color idleColor: "#a6e3a1"
    readonly property color backupColor: "#f9e2af"
    readonly property color alertColor: "#f38ba8"
    readonly property color offlineColor: "#6c7086"

    function statusColor() {
        if (statusText === "backup" || statusText === "running")
            return backupColor
        if (statusText === "alert" || statusText === "error" || hasAlert)
            return alertColor
        if (statusText === "offline" || statusText === "unknown")
            return offlineColor
        return idleColor
    }

    function statusIcon() {
        if (statusText === "backup" || statusText === "running")
            return "⟳"
        if (statusText === "alert" || statusText === "error" || hasAlert)
            return "⚠"
        if (statusText === "offline" || statusText === "unknown")
            return "○"
        return "●"
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: "#313244"
        border.color: parent.statusColor()
        border.width: 2

        RowLayout {
            anchors.centerIn: parent
            spacing: 12

            Text {
                text: parent.parent.statusIcon()
                font.pixelSize: 36
                color: parent.parent.statusColor()
            }

            Column {
                spacing: 4

                Text {
                    text: "Agent Status"
                    font.pixelSize: 12
                    color: "#6c7086"
                }

                Text {
                    text: statusText.charAt(0).toUpperCase() + statusText.slice(1)
                    font.pixelSize: 24
                    font.bold: true
                    color: statusColor()
                }
            }
        }
    }
}
