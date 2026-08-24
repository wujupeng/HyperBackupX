package api

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"

	"hbx-control/internal/auth"
	"hbx-control/internal/rbac"
)

func init() {
	gin.SetMode(gin.TestMode)
}

func TestTraceIDMiddleware(t *testing.T) {
	r := gin.New()
	r.Use(TraceIDMiddleware())
	r.GET("/test", func(c *gin.Context) {
		tid := c.GetString(ContextKeyTraceID)
		if tid == "" {
			t.Error("trace ID should be set")
		}
		c.JSON(http.StatusOK, gin.H{"trace_id": tid})
	})

	req := httptest.NewRequest("GET", "/test", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Header().Get("X-Trace-Id") == "" {
		t.Error("X-Trace-Id header should be set")
	}
}

func TestTraceIDMiddlewarePreservesExisting(t *testing.T) {
	r := gin.New()
	r.Use(TraceIDMiddleware())
	r.GET("/test", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{})
	})

	existingID := "existing-trace-id"
	req := httptest.NewRequest("GET", "/test", nil)
	req.Header.Set("X-Trace-Id", existingID)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Header().Get("X-Trace-Id") != existingID {
		t.Errorf("expected %s, got %s", existingID, w.Header().Get("X-Trace-Id"))
	}
}

func TestJWTAuthMiddlewareValidToken(t *testing.T) {
	jwtMgr := auth.NewJWTManager()
	userID := uuid.New()
	token, _ := jwtMgr.Generate(userID, "testuser", []string{"admin"})

	r := gin.New()
	r.Use(JWTAuthMiddleware(jwtMgr))
	r.GET("/protected", func(c *gin.Context) {
		claims := getClaims(c)
		c.JSON(http.StatusOK, gin.H{"user_id": claims.UserID})
	})

	req := httptest.NewRequest("GET", "/protected", nil)
	req.Header.Set("Authorization", "Bearer "+token)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestJWTAuthMiddlewareMissingHeader(t *testing.T) {
	jwtMgr := auth.NewJWTManager()

	r := gin.New()
	r.Use(JWTAuthMiddleware(jwtMgr))
	r.GET("/protected", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{})
	})

	req := httptest.NewRequest("GET", "/protected", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", w.Code)
	}
}

func TestJWTAuthMiddlewareInvalidToken(t *testing.T) {
	jwtMgr := auth.NewJWTManager()

	r := gin.New()
	r.Use(JWTAuthMiddleware(jwtMgr))
	r.GET("/protected", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{})
	})

	req := httptest.NewRequest("GET", "/protected", nil)
	req.Header.Set("Authorization", "Bearer invalid.token.here")
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", w.Code)
	}
}

func TestRBACMiddlewareAuthorized(t *testing.T) {
	jwtMgr := auth.NewJWTManager()
	userID := uuid.New()
	token, _ := jwtMgr.Generate(userID, "admin_user", []string{"*"})

	r := gin.New()
	r.Use(JWTAuthMiddleware(jwtMgr), RBACMiddleware(rbac.PermUsersAll))
	r.GET("/admin-only", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"status": "ok"})
	})

	req := httptest.NewRequest("GET", "/admin-only", nil)
	req.Header.Set("Authorization", "Bearer "+token)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("admin should access, got %d", w.Code)
	}
}

func TestRBACMiddlewareForbidden(t *testing.T) {
	jwtMgr := auth.NewJWTManager()
	userID := uuid.New()
	token, _ := jwtMgr.Generate(userID, "auditor_user", []string{"audit:read"})

	r := gin.New()
	r.Use(JWTAuthMiddleware(jwtMgr), RBACMiddleware(rbac.PermUsersAll))
	r.GET("/admin-only", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"status": "ok"})
	})

	req := httptest.NewRequest("GET", "/admin-only", nil)
	req.Header.Set("Authorization", "Bearer "+token)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusForbidden {
		t.Errorf("auditor should be forbidden, got %d", w.Code)
	}
}

func TestCORSMiddleware(t *testing.T) {
	r := gin.New()
	r.Use(CORSMiddleware())
	r.GET("/test", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{})
	})

	req := httptest.NewRequest("OPTIONS", "/test", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNoContent {
		t.Errorf("OPTIONS should return 204, got %d", w.Code)
	}
	if w.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Error("CORS origin header missing")
	}
}

func TestRateLimitMiddleware(t *testing.T) {
	r := gin.New()
	r.Use(RateLimitMiddleware(1, 1))
	r.GET("/test", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{})
	})

	req1 := httptest.NewRequest("GET", "/test", nil)
	w1 := httptest.NewRecorder()
	r.ServeHTTP(w1, req1)
	if w1.Code != http.StatusOK {
		t.Errorf("first request should succeed, got %d", w1.Code)
	}

	req2 := httptest.NewRequest("GET", "/test", nil)
	w2 := httptest.NewRecorder()
	r.ServeHTTP(w2, req2)
	if w2.Code != http.StatusTooManyRequests {
		t.Errorf("second request should be rate limited, got %d", w2.Code)
	}
}

func TestClaimsType(t *testing.T) {
	claims := &auth.Claims{
		UserID:   "test-user-id",
		Username: "testuser",
		Roles:    []string{"admin"},
		RegisteredClaims: jwt.RegisteredClaims{
			Subject: "test-user-id",
		},
	}
	if claims.UserID != "test-user-id" {
		t.Error("claims UserID mismatch")
	}
	if len(claims.Roles) != 1 {
		t.Error("claims Roles count mismatch")
	}
}