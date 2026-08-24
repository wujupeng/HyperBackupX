#pragma once

#include <QObject>
#include <QString>
#include <QJsonObject>
#include <QJsonArray>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QTimer>

class TrayIPC : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString agentStatus READ agentStatus NOTIFY statusChanged)
    Q_PROPERTY(QString agentVersion READ agentVersion NOTIFY statusChanged)
    Q_PROPERTY(QString lastBackupTime READ lastBackupTime NOTIFY statusChanged)
    Q_PROPERTY(bool connected READ connected NOTIFY statusChanged)

public:
    explicit TrayIPC(QObject *parent = nullptr);
    ~TrayIPC() override;

    QString agentStatus() const { return m_agentStatus; }
    QString agentVersion() const { return m_agentVersion; }
    QString lastBackupTime() const { return m_lastBackupTime; }
    bool connected() const { return m_connected; }

    Q_INVOKABLE void refreshStatus();
    Q_INVOKABLE void triggerBackup(const QString &jobId);
    Q_INVOKABLE void pauseTask(const QString &jobId);
    Q_INVOKABLE void resumeTask(const QString &jobId);
    Q_INVOKABLE void getJobList();

signals:
    void statusChanged();
    void jobListReceived(const QJsonArray &jobs);
    void errorOccurred(const QString &message);

private slots:
    void onReplyFinished(QNetworkReply *reply);
    void onPollTimer();

private:
    void sendRequest(const QString &endpoint, const QString &method = "GET",
                     const QJsonObject &body = QJsonObject());

    QNetworkAccessManager *m_manager;
    QTimer *m_pollTimer;
    QString m_baseUrl;

    QString m_agentStatus;
    QString m_agentVersion;
    QString m_lastBackupTime;
    bool m_connected;

    QHash<QNetworkReply *, QString> m_pendingRequests;
};
