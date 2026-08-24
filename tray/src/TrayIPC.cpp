#include "TrayIPC.h"
#include <QNetworkRequest>
#include <QJsonDocument>
#include <QByteArray>
#include <QUrl>

TrayIPC::TrayIPC(QObject *parent)
    : QObject(parent)
    , m_manager(new QNetworkAccessManager(this))
    , m_pollTimer(new QTimer(this))
    , m_baseUrl("http://127.0.0.1:18080")
    , m_agentStatus("unknown")
    , m_agentVersion("")
    , m_lastBackupTime("")
    , m_connected(false)
{
    connect(m_manager, &QNetworkAccessManager::finished,
            this, &TrayIPC::onReplyFinished);
    connect(m_pollTimer, &QTimer::timeout,
            this, &TrayIPC::onPollTimer);

    m_pollTimer->start(5000);
    refreshStatus();
}

TrayIPC::~TrayIPC() = default;

void TrayIPC::refreshStatus()
{
    sendRequest("/api/v1/agent/status");
}

void TrayIPC::triggerBackup(const QString &jobId)
{
    QJsonObject body;
    body["job_id"] = jobId;
    sendRequest("/api/v1/agent/trigger-backup", "POST", body);
}

void TrayIPC::pauseTask(const QString &jobId)
{
    QJsonObject body;
    body["job_id"] = jobId;
    sendRequest("/api/v1/agent/pause-task", "POST", body);
}

void TrayIPC::resumeTask(const QString &jobId)
{
    QJsonObject body;
    body["job_id"] = jobId;
    sendRequest("/api/v1/agent/resume-task", "POST", body);
}

void TrayIPC::getJobList()
{
    sendRequest("/api/v1/agent/jobs");
}

void TrayIPC::sendRequest(const QString &endpoint, const QString &method,
                           const QJsonObject &body)
{
    QUrl url(m_baseUrl + endpoint);
    QNetworkRequest request(url);
    request.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");

    QByteArray httpMethod = method.toUtf8();
    QByteArray data;
    if (method == "POST" || method == "PUT") {
        data = QJsonDocument(body).toJson(QJsonDocument::Compact);
    }

    QNetworkReply *reply = m_manager->sendCustomRequest(request, httpMethod, data);
    m_pendingRequests[reply] = endpoint;
}

void TrayIPC::onReplyFinished(QNetworkReply *reply)
{
    QString endpoint = m_pendingRequests.value(reply, "");
    m_pendingRequests.remove(reply);

    reply->deleteLater();

    if (reply->error() != QNetworkReply::NoError) {
        m_connected = false;
        m_agentStatus = "offline";
        emit statusChanged();
        emit errorOccurred(reply->errorString());
        return;
    }

    m_connected = true;
    QByteArray responseData = reply->readAll();
    QJsonDocument doc = QJsonDocument::fromJson(responseData);

    if (endpoint == "/api/v1/agent/status") {
        QJsonObject obj = doc.object();
        m_agentStatus = obj.value("status").toString("unknown");
        m_agentVersion = obj.value("agent_version").toString("");
        m_lastBackupTime = obj.value("last_backup").toString("");
        emit statusChanged();
    } else if (endpoint == "/api/v1/agent/jobs") {
        QJsonArray jobs = doc.array();
        emit jobListReceived(jobs);
    }
}

void TrayIPC::onPollTimer()
{
    refreshStatus();
}
