package handler

import (
	"net/http"
	"strings"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"

	"hbx-control/internal/auth"
	"hbx-control/internal/badou/model"
	"hbx-control/internal/badou/service"
)

type Handler struct {
	svc     *service.Service
	audit   *service.AuditForwarder
}

func NewHandler(svc *service.Service, audit *service.AuditForwarder) *Handler {
	return &Handler{svc: svc, audit: audit}
}

func (h *Handler) actorID(c *gin.Context) string {
	if claims, exists := c.Get("hbx.claims"); exists {
		if cl, ok := claims.(*auth.Claims); ok {
			return cl.UserID
		}
	}
	return "system"
}

func (h *Handler) traceID(c *gin.Context) string {
	return c.GetString("hbx.trace_id")
}

func (h *Handler) ListRepositories(c *gin.Context) {
	repos, err := h.svc.ListRepositories(c.Request.Context())
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to list repositories"})
		return
	}
	if repos == nil {
		repos = []model.Repository{}
	}
	c.JSON(http.StatusOK, gin.H{"repositories": repos})
}

func (h *Handler) CreateRepository(c *gin.Context) {
	var req model.CreateRepositoryRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request: " + err.Error()})
		return
	}
	repo, err := h.svc.CreateRepository(c.Request.Context(), req)
	if err != nil {
		if strings.Contains(err.Error(), "duplicate") || strings.Contains(err.Error(), "unique") {
			c.JSON(http.StatusConflict, gin.H{"error": "repository name already exists"})
			return
		}
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create repository"})
		return
	}
	h.audit.ForwardRepositoryCreate(c.Request.Context(), h.actorID(c), repo.ID.String(), h.traceID(c))
	c.JSON(http.StatusCreated, repo)
}

func (h *Handler) GetRepository(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	repo, err := h.svc.GetRepository(c.Request.Context(), id)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "repository not found"})
		return
	}
	c.JSON(http.StatusOK, repo)
}

func (h *Handler) UpdateRepository(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	var req model.UpdateRepositoryRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request: " + err.Error()})
		return
	}
	if err := h.svc.UpdateRepository(c.Request.Context(), id, req); err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to update repository"})
		return
	}
	c.JSON(http.StatusOK, gin.H{"status": "updated"})
}

func (h *Handler) DeleteRepository(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	if err := h.svc.DeleteRepository(c.Request.Context(), id); err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to delete repository"})
		return
	}
	h.audit.ForwardRepositoryDelete(c.Request.Context(), h.actorID(c), id.String(), h.traceID(c))
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (h *Handler) SetImmutable(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	var req model.SetImmutableRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request: " + err.Error()})
		return
	}
	if err := h.svc.SetImmutableRetention(c.Request.Context(), id, req.RetentionDays); err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to set immutable retention"})
		return
	}
	h.audit.ForwardImmutableSet(c.Request.Context(), h.actorID(c), id.String(), req.RetentionDays, h.traceID(c))
	c.JSON(http.StatusOK, gin.H{"status": "set", "retention_days": req.RetentionDays})
}

func (h *Handler) ListVersions(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	versions, err := h.svc.ListVersions(c.Request.Context(), id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to list versions"})
		return
	}
	if versions == nil {
		versions = []model.Version{}
	}
	c.JSON(http.StatusOK, gin.H{"versions": versions})
}

func (h *Handler) GetVersion(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	vid := c.Param("vid")
	versions, err := h.svc.ListVersions(c.Request.Context(), id)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to get version"})
		return
	}
	for _, v := range versions {
		if v.VersionID == vid {
			c.JSON(http.StatusOK, v)
			return
		}
	}
	c.JSON(http.StatusNotFound, gin.H{"error": "version not found"})
}

func (h *Handler) DeleteVersion(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	vid := c.Param("vid")
	if err := h.svc.DeleteVersion(c.Request.Context(), id, vid); err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to delete version"})
		return
	}
	h.audit.ForwardVersionDelete(c.Request.Context(), h.actorID(c), id.String(), vid, h.traceID(c))
	c.JSON(http.StatusOK, gin.H{"status": "deleted"})
}

func (h *Handler) VerifyRepository(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	var req model.VerifyRequest
	_ = c.ShouldBindJSON(&req)
	result, err := h.svc.VerifyRepository(c.Request.Context(), id, req.Level)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "verify failed"})
		return
	}
	h.audit.ForwardVerify(c.Request.Context(), h.actorID(c), id.String(), h.traceID(c))
	c.JSON(http.StatusOK, result)
}

func (h *Handler) TriggerGC(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	report, err := h.svc.TriggerGC(c.Request.Context(), id, h.actorID(c))
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "gc failed"})
		return
	}
	h.audit.ForwardGC(c.Request.Context(), h.actorID(c), id.String(), h.traceID(c))
	c.JSON(http.StatusOK, report)
}

func (h *Handler) GetGCReport(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid repository id"})
		return
	}
	report, err := h.svc.GetGCReport(c.Request.Context(), id)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "no gc report found"})
		return
	}
	c.JSON(http.StatusOK, report)
}

func (h *Handler) ListNodes(c *gin.Context) {
	nodes, err := h.svc.ListNodes(c.Request.Context())
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to list nodes"})
		return
	}
	if nodes == nil {
		nodes = []model.Node{}
	}
	c.JSON(http.StatusOK, gin.H{"nodes": nodes})
}

func (h *Handler) AddNode(c *gin.Context) {
	var req model.AddNodeRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request: " + err.Error()})
		return
	}
	node, err := h.svc.AddNode(c.Request.Context(), req)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to add node"})
		return
	}
	h.audit.ForwardNodeAdd(c.Request.Context(), h.actorID(c), node.ID.String(), h.traceID(c))
	c.JSON(http.StatusCreated, node)
}

func (h *Handler) RemoveNode(c *gin.Context) {
	id, err := uuid.Parse(c.Param("id"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid node id"})
		return
	}
	if err := h.svc.RemoveNode(c.Request.Context(), id); err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to remove node"})
		return
	}
	h.audit.ForwardNodeRemove(c.Request.Context(), h.actorID(c), id.String(), h.traceID(c))
	c.JSON(http.StatusOK, gin.H{"status": "removed"})
}

func (h *Handler) ClusterHealth(c *gin.Context) {
	health, err := h.svc.GetClusterHealth(c.Request.Context(), "", 0)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to get cluster health"})
		return
	}
	c.JSON(http.StatusOK, health)
}

func (h *Handler) ExpandCapacity(c *gin.Context) {
	var req model.CapacityRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request: " + err.Error()})
		return
	}
	c.JSON(http.StatusNotImplemented, gin.H{
		"error":   "capacity expansion is not supported in Phase BD-21",
		"node_id": req.NodeID,
		"hint":    "use cluster-join.sh to add a new node instead of expanding an existing node",
	})
}