package handler

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"

	"hbx-control/internal/audit"
	"hbx-control/internal/auth"
	"hbx-control/internal/badou/repository"
	"hbx-control/internal/badou/service"
)

func init() {
	gin.SetMode(gin.TestMode)
}

func newTestHandler() *Handler {
	repo := repository.NewBadouRepo(nil)
	svc := service.NewService(repo, service.NewStubClient())
	auditFwd := service.NewAuditForwarder(audit.NewLogger(nil))
	return NewHandler(svc, auditFwd)
}

func newTestRouter() (*gin.Engine, *Handler) {
	h := newTestHandler()
	r := gin.New()
	return r, h
}

func authMiddleware() gin.HandlerFunc {
	jwtMgr, _ := auth.NewJWTManager([]byte("test-secret-that-is-long-enough-32+bytes!"))
	return func(c *gin.Context) {
		token, _ := jwtMgr.Generate(uuid.New(), "testuser", []string{"*"})
		c.Set("hbx.claims", &auth.Claims{
			UserID:   "test-user-id",
			Username: "testuser",
			Roles:    []string{"*"},
			RegisteredClaims: jwt.RegisteredClaims{
				Subject: "test-user-id",
			},
		})
		_ = token
		c.Next()
	}
}

func TestListRepositoriesDBError(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.GET("/api/v1/badou/repositories", h.ListRepositories)

	req := httptest.NewRequest("GET", "/api/v1/badou/repositories", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Errorf("expected 500 (nil pool), got %d", w.Code)
	}
}

func TestGetRepositoryInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.GET("/api/v1/badou/repositories/:id", h.GetRepository)

	req := httptest.NewRequest("GET", "/api/v1/badou/repositories/invalid-uuid", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestDeleteRepositoryInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.DELETE("/api/v1/badou/repositories/:id", h.DeleteRepository)

	req := httptest.NewRequest("DELETE", "/api/v1/badou/repositories/not-a-uuid", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestSetImmutableInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.POST("/api/v1/badou/repositories/:id/immutable", h.SetImmutable)

	req := httptest.NewRequest("POST", "/api/v1/badou/repositories/bad-id/immutable", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestListVersionsInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.GET("/api/v1/badou/repositories/:id/versions", h.ListVersions)

	req := httptest.NewRequest("GET", "/api/v1/badou/repositories/xxx/versions", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestVerifyRepositoryInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.POST("/api/v1/badou/repositories/:id/verify", h.VerifyRepository)

	req := httptest.NewRequest("POST", "/api/v1/badou/repositories/xxx/verify", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestTriggerGCInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.POST("/api/v1/badou/repositories/:id/gc", h.TriggerGC)

	req := httptest.NewRequest("POST", "/api/v1/badou/repositories/xxx/gc", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestRemoveNodeInvalidID(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.DELETE("/api/v1/badou/cluster/nodes/:id", h.RemoveNode)

	req := httptest.NewRequest("DELETE", "/api/v1/badou/cluster/nodes/bad-id", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid UUID, got %d", w.Code)
	}
}

func TestClusterHealth(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.GET("/api/v1/badou/cluster/health", h.ClusterHealth)

	req := httptest.NewRequest("GET", "/api/v1/badou/cluster/health", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestExpandCapacityInvalidBody(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.POST("/api/v1/badou/cluster/capacity", h.ExpandCapacity)

	req := httptest.NewRequest("POST", "/api/v1/badou/cluster/capacity", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for missing body, got %d", w.Code)
	}
}

func TestExpandCapacityReturns501(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.POST("/api/v1/badou/cluster/capacity", h.ExpandCapacity)

	body := `{"node_id":"node-1","additional_bytes":1073741824}`
	req := httptest.NewRequest("POST", "/api/v1/badou/cluster/capacity", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotImplemented {
		t.Errorf("expected 501 Not Implemented, got %d", w.Code)
	}
}

func TestActorIDWithClaims(t *testing.T) {
	r, h := newTestRouter()
	r.Use(authMiddleware())
	r.GET("/test", func(c *gin.Context) {
		id := h.actorID(c)
		if id != "test-user-id" {
			t.Errorf("expected test-user-id, got %s", id)
		}
		c.JSON(http.StatusOK, gin.H{"actor_id": id})
	})

	req := httptest.NewRequest("GET", "/test", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestActorIDWithoutClaims(t *testing.T) {
	r, h := newTestRouter()
	r.GET("/test", func(c *gin.Context) {
		id := h.actorID(c)
		if id != "system" {
			t.Errorf("expected system, got %s", id)
		}
		c.JSON(http.StatusOK, gin.H{"actor_id": id})
	})

	req := httptest.NewRequest("GET", "/test", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestAuditForwarderNilLogger(t *testing.T) {
	fwd := service.NewAuditForwarder(audit.NewLogger(nil))
	fwd.ForwardRepositoryCreate(context.Background(), "user1", "repo1", "trace1")
	fwd.ForwardRepositoryDelete(context.Background(), "user1", "repo1", "trace1")
	fwd.ForwardImmutableSet(context.Background(), "user1", "repo1", 30, "trace1")
	fwd.ForwardVersionDelete(context.Background(), "user1", "repo1", "v1", "trace1")
	fwd.ForwardVerify(context.Background(), "user1", "repo1", "trace1")
	fwd.ForwardGC(context.Background(), "user1", "repo1", "trace1")
	fwd.ForwardNodeAdd(context.Background(), "user1", "node1", "trace1")
	fwd.ForwardNodeRemove(context.Background(), "user1", "node1", "trace1")
	fwd.ForwardCapacityExpand(context.Background(), "user1", "node1", 1024, "trace1")
}
