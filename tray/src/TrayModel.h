#pragma once

#include <QObject>
#include <QString>
#include <QList>
#include <QJsonObject>
#include <QAbstractListModel>

struct JobInfo {
    QString jobId;
    QString name;
    QString status;
    QString startTime;
    QString completedTime;
    quint64 bytesProcessed;
    quint64 bytesStored;
    quint32 fileCount;
};

class TrayModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(QString agentStatus READ agentStatus WRITE setAgentStatus NOTIFY agentStatusChanged)
    Q_PROPERTY(QString agentVersion READ agentVersion WRITE setAgentVersion NOTIFY agentStatusChanged)
    Q_PROPERTY(QString lastBackupTime READ lastBackupTime WRITE setLastBackupTime NOTIFY agentStatusChanged)
    Q_PROPERTY(bool hasAlert READ hasAlert NOTIFY agentStatusChanged)

public:
    enum Roles {
        JobIdRole = Qt::UserRole + 1,
        NameRole,
        StatusRole,
        StartTimeRole,
        CompletedTimeRole,
        BytesProcessedRole,
        BytesStoredRole,
        FileCountRole
    };

    explicit TrayModel(QObject *parent = nullptr);
    ~TrayModel() override;

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const override;
    QHash<int, QByteArray> roleNames() const override;

    QString agentStatus() const { return m_agentStatus; }
    QString agentVersion() const { return m_agentVersion; }
    QString lastBackupTime() const { return m_lastBackupTime; }
    bool hasAlert() const { return m_agentStatus == "alert" || m_agentStatus == "error"; }

    void setAgentStatus(const QString &status);
    void setAgentVersion(const QString &version);
    void setLastBackupTime(const QString &time);

    Q_INVOKABLE void updateJobs(const QJsonArray &jobs);
    Q_INVOKABLE void clearJobs();
    Q_INVOKABLE QString sanitizeText(const QString &input) const;

signals:
    void agentStatusChanged();

private:
    QString m_agentStatus;
    QString m_agentVersion;
    QString m_lastBackupTime;
    QList<JobInfo> m_jobs;
};
