#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QAction>
#include <QIcon>
#include <QQuickWindow>

#include "TrayIPC.h"
#include "TrayModel.h"

int main(int argc, char *argv[])
{
    QGuiApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
    QGuiApplication app(argc, argv);

    app.setApplicationName("HyperBackup X Tray");
    app.setOrganizationName("HyperBackupX");

    TrayIPC ipc;
    TrayModel model;

    QObject::connect(&ipc, &TrayIPC::statusChanged, [&]() {
        model.setAgentStatus(ipc.agentStatus());
        model.setAgentVersion(ipc.agentVersion());
        model.setLastBackupTime(ipc.lastBackupTime());
    });

    QObject::connect(&ipc, &TrayIPC::jobListReceived, [&](const QJsonArray &jobs) {
        model.updateJobs(jobs);
    });

    QSystemTrayIcon trayIcon;
    trayIcon.setToolTip("HyperBackup X Agent");

    QMenu trayMenu;
    QAction *statusAction = trayMenu.addAction("Status: Idle");
    statusAction->setEnabled(false);
    trayMenu.addSeparator();
    QAction *showAction = trayMenu.addAction("Show Details");
    QAction *triggerBackupAction = trayMenu.addAction("Trigger Backup");
    trayMenu.addSeparator();
    QAction *quitAction = trayMenu.addAction("Quit");

    trayIcon.setContextMenu(&trayMenu);
    trayIcon.show();

    QObject::connect(&ipc, &TrayIPC::statusChanged, [&]() {
        QString statusText = "Status: " + ipc.agentStatus();
        statusAction->setText(statusText);
        trayIcon.setToolTip("HyperBackup X - " + ipc.agentStatus());
    });

    QQmlApplicationEngine engine;

    qmlRegisterSingletonInstance("HyperBackupX.Tray", 1, 0, "TrayIPC", &ipc);
    qmlRegisterSingletonInstance("HyperBackupX.Tray", 1, 0, "TrayModel", &model);

    engine.load(QUrl(QStringLiteral("qrc:/HyperBackupX/Tray/qml/main.qml")));

    QObject::connect(showAction, &QAction::triggered, [&]() {
        if (!engine.rootObjects().isEmpty()) {
            auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
            if (window) {
                window->show();
                window->raise();
                window->requestActivate();
            }
        }
    });

    QObject::connect(triggerBackupAction, &QAction::triggered, [&]() {
        ipc.triggerBackup("");
    });

    QObject::connect(quitAction, &QAction::triggered, &app, &QGuiApplication::quit);

    return app.exec();
}
