package soak

import (
	"net/http"

	"github.com/gin-gonic/gin"

	"hbx-control/internal/certorch"
	"hbx-control/internal/certorch/common"
)

type APIHandler struct {
	orchestrator *certorch.CertOrchestrator
}

func NewAPIHandler(orchestrator *certorch.CertOrchestrator) *APIHandler {
	return &APIHandler{orchestrator: orchestrator}
}

func (h *APIHandler) StartSoak(c *gin.Context) {
	var req SoakStartRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if !req.Duration.Valid() {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid duration, must be 24h/72h/7d"})
		return
	}

	reqJSON, _ := jsonMarshal(req)
	sessionID, err := h.orchestrator.StartGate(c.Request.Context(), common.GateG17Soak, req.Operator, reqJSON)
	if err != nil {
		if err == common.ErrSessionAlreadyActive {
			c.JSON(http.StatusConflict, gin.H{"error": "soak test already active"})
			return
		}
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, gin.H{"session_id": sessionID, "status": "running"})
}

func (h *APIHandler) QuerySoak(c *gin.Context) {
	sessionID := c.Param("sessionId")
	session, err := h.orchestrator.QuerySession(c.Request.Context(), sessionID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": err.Error()})
		return
	}
	c.JSON(http.StatusOK, session)
}

func (h *APIHandler) DownloadSoakReport(c *gin.Context) {
	sessionID := c.Param("sessionId")
	report, err := h.orchestrator.DownloadReport(c.Request.Context(), sessionID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": err.Error()})
		return
	}
	c.JSON(http.StatusOK, report)
}

func jsonMarshal(v interface{}) ([]byte, error) {
	return jsonMarshalImpl(v)
}