package service

import (
	"context"

	"hbx-control/internal/audit"
)

type AuditForwarder struct {
	logger *audit.Logger
}

func NewAuditForwarder(logger *audit.Logger) *AuditForwarder {
	return &AuditForwarder{logger: logger}
}

func (f *AuditForwarder) ForwardRepositoryCreate(ctx context.Context, actorID, repoID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.repository.create",
		TargetType: "badou_repository",
		TargetID:   repoID,
		Result:     "success",
		TraceID:    traceID,
	})
}

func (f *AuditForwarder) ForwardRepositoryDelete(ctx context.Context, actorID, repoID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.repository.delete",
		TargetType: "badou_repository",
		TargetID:   repoID,
		Result:     "success",
		TraceID:    traceID,
	})
}

func (f *AuditForwarder) ForwardImmutableSet(ctx context.Context, actorID, repoID string, days int, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.repository.immutable",
		TargetType: "badou_repository",
		TargetID:   repoID,
		Result:     "success",
		TraceID:    traceID,
		Detail: map[string]interface{}{
			"retention_days": days,
		},
	})
}

func (f *AuditForwarder) ForwardVersionDelete(ctx context.Context, actorID, repoID, versionID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.version.delete",
		TargetType: "badou_version",
		TargetID:   versionID,
		Result:     "success",
		TraceID:    traceID,
		Detail: map[string]interface{}{
			"repo_id": repoID,
		},
	})
}

func (f *AuditForwarder) ForwardVerify(ctx context.Context, actorID, repoID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.verify",
		TargetType: "badou_repository",
		TargetID:   repoID,
		Result:     "success",
		TraceID:    traceID,
	})
}

func (f *AuditForwarder) ForwardGC(ctx context.Context, actorID, repoID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.gc",
		TargetType: "badou_repository",
		TargetID:   repoID,
		Result:     "success",
		TraceID:    traceID,
	})
}

func (f *AuditForwarder) ForwardNodeAdd(ctx context.Context, actorID, nodeID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.cluster.node_add",
		TargetType: "badou_node",
		TargetID:   nodeID,
		Result:     "success",
		TraceID:    traceID,
	})
}

func (f *AuditForwarder) ForwardNodeRemove(ctx context.Context, actorID, nodeID string, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.cluster.node_remove",
		TargetType: "badou_node",
		TargetID:   nodeID,
		Result:     "success",
		TraceID:    traceID,
	})
}

func (f *AuditForwarder) ForwardCapacityExpand(ctx context.Context, actorID, nodeID string, additionalBytes int64, traceID string) {
	f.logger.Record(ctx, audit.Entry{
		ActorID:    actorID,
		ActorType:  audit.ActorTypeUser,
		Action:     "badou.cluster.capacity",
		TargetType: "badou_node",
		TargetID:   nodeID,
		Result:     "success",
		TraceID:    traceID,
		Detail: map[string]interface{}{
			"additional_bytes": additionalBytes,
		},
	})
}