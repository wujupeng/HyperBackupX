#include "TrayModel.h"
#include <QRegularExpression>
#include <QRegularExpressionMatch>

TrayModel::TrayModel(QObject *parent)
    : QAbstractListModel(parent)
    , m_agentStatus("idle")
{
}

TrayModel::~TrayModel() = default;

int TrayModel::rowCount(const QModelIndex &parent) const
{
    if (parent.isValid())
        return 0;
    return m_jobs.size();
}

QVariant TrayModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() < 0 || index.row() >= m_jobs.size())
        return QVariant();

    const JobInfo &job = m_jobs[index.row()];
    switch (role) {
    case JobIdRole:
        return job.jobId;
    case NameRole:
        return job.name;
    case StatusRole:
        return job.status;
    case StartTimeRole:
        return job.startTime;
    case CompletedTimeRole:
        return job.completedTime;
    case BytesProcessedRole:
        return static_cast<quint64>(job.bytesProcessed);
    case BytesStoredRole:
        return static_cast<quint64>(job.bytesStored);
    case FileCountRole:
        return static_cast<quint32>(job.fileCount);
    default:
        return QVariant();
    }
}

QHash<int, QByteArray> TrayModel::roleNames() const
{
    QHash<int, QByteArray> roles;
    roles[JobIdRole] = "jobId";
    roles[NameRole] = "name";
    roles[StatusRole] = "status";
    roles[StartTimeRole] = "startTime";
    roles[CompletedTimeRole] = "completedTime";
    roles[BytesProcessedRole] = "bytesProcessed";
    roles[BytesStoredRole] = "bytesStored";
    roles[FileCountRole] = "fileCount";
    return roles;
}

void TrayModel::setAgentStatus(const QString &status)
{
    if (m_agentStatus != status) {
        m_agentStatus = status;
        emit agentStatusChanged();
    }
}

void TrayModel::setAgentVersion(const QString &version)
{
    if (m_agentVersion != version) {
        m_agentVersion = version;
        emit agentStatusChanged();
    }
}

void TrayModel::setLastBackupTime(const QString &time)
{
    if (m_lastBackupTime != time) {
        m_lastBackupTime = time;
        emit agentStatusChanged();
    }
}

void TrayModel::updateJobs(const QJsonArray &jobs)
{
    beginResetModel();
    m_jobs.clear();
    for (const auto &jobVal : jobs) {
        QJsonObject jobObj = jobVal.toObject();
        JobInfo info;
        info.jobId = jobObj.value("job_id").toString();
        info.name = jobObj.value("name").toString();
        info.status = jobObj.value("status").toString();
        info.startTime = jobObj.value("started_at").toString();
        info.completedTime = jobObj.value("completed_at").toString();
        info.bytesProcessed = static_cast<quint64>(jobObj.value("bytes_processed").toDouble());
        info.bytesStored = static_cast<quint64>(jobObj.value("bytes_stored").toDouble());
        info.fileCount = static_cast<quint32>(jobObj.value("file_count").toInt());
        m_jobs.append(info);
    }
    endResetModel();
}

void TrayModel::clearJobs()
{
    beginResetModel();
    m_jobs.clear();
    endResetModel();
}

QString TrayModel::sanitizeText(const QString &input) const
{
    static const QRegularExpression sensitivePattern(
        QStringLiteral("(?i)(password|passwd|pwd|secret|token|api_key|apikey|private_key|credential)\\s*[=:]\\s*\\S+")
    );
    static const QRegularExpression bearerPattern(
        QStringLiteral("(?i)(bearer)\\s+[A-Za-z0-9\\-_\\.]+")
    );

    QString result = input;
    result.replace(sensitivePattern, QStringLiteral("***REDACTED***"));
    result.replace(bearerPattern, QStringLiteral("***REDACTED***"));
    return result;
}
