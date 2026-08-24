package api

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
	"golang.org/x/time/rate"

	"hbx-control/internal/audit"
	"hbx-control/internal/auth"
	"hbx-control/internal/rbac"
)

const (
	ContextKeyClaims  = "hbx.claims"
	ContextKeyTraceID = "hbx.trace_id"
)

func TraceIDMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		traceID := c.GetHeader("X-Trace-Id")
		if traceID == "" {
			traceID = uuid.New().String()
		}
		c.Set(ContextKeyTraceID, traceID)
		c.Header("X-Trace-Id", traceID)
		c.Next()
	}
}

func RequestLogMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		start := time.Now()
		c.Next()
		slog.Info("http request",
			"method", c.Request.Method,
			"path", c.Request.URL.Path,
			"status", c.Writer.Status(),
			"duration", time.Since(start).String(),
			"trace_id", c.GetString(ContextKeyTraceID),
		)
	}
}

func RateLimitMiddleware(rps float64, burst int) gin.HandlerFunc {
	limiter := rate.NewLimiter(rate.Limit(rps), burst)
	return func(c *gin.Context) {
		if !limiter.Allow() {
			c.JSON(http.StatusTooManyRequests, gin.H{"error": "rate limit exceeded"})
			c.Abort()
			return
		}
		c.Next()
	}
}

func JWTAuthMiddleware(jwtMgr *auth.JWTManager) gin.HandlerFunc {
	return func(c *gin.Context) {
		header := c.GetHeader("Authorization")
		if header == "" {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "missing authorization header"})
			c.Abort()
			return
		}
		if !strings.HasPrefix(header, "Bearer ") {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid authorization scheme"})
			c.Abort()
			return
		}
		tokenStr := strings.TrimPrefix(header, "Bearer ")
		claims, err := jwtMgr.Verify(tokenStr)
		if err != nil {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid token"})
			c.Abort()
			return
		}
		c.Set(ContextKeyClaims, claims)
		c.Next()
	}
}

func RBACMiddleware(required rbac.Permission) gin.HandlerFunc {
	return func(c *gin.Context) {
		claimsVal, exists := c.Get(ContextKeyClaims)
		if !exists {
			c.JSON(http.StatusForbidden, gin.H{"error": "no claims in context"})
			c.Abort()
			return
		}
		claims := claimsVal.(*auth.Claims)
		if !rbac.Check(claims.Roles, required) {
			c.JSON(http.StatusForbidden, gin.H{"error": "insufficient permissions"})
			c.Abort()
			return
		}
		c.Next()
	}
}

func AuditMiddleware(auditLogger *audit.Logger, action, targetType string) gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Next()

		claimsVal, _ := c.Get(ContextKeyClaims)
		var actorID, actorType string
		if claims, ok := claimsVal.(*auth.Claims); ok {
			actorID = claims.UserID
			actorType = string(audit.ActorTypeUser)
		} else {
			actorID = "anonymous"
			actorType = string(audit.ActorTypeSystem)
		}

		result := "success"
		if c.Writer.Status() >= 400 {
			result = "failed"
		}

		entry := audit.Entry{
			ActorID:    actorID,
			ActorType:  audit.ActorType(actorType),
			Action:     action,
			TargetType: targetType,
			TargetID:   c.Param("id"),
			Result:     result,
			TraceID:    c.GetString(ContextKeyTraceID),
		}

		go auditLogger.Record(context.Background(), entry)
	}
}

func CORSMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Header("Access-Control-Allow-Origin", "*")
		c.Header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		c.Header("Access-Control-Allow-Headers", "Origin, Content-Type, Authorization, X-Trace-Id")
		c.Header("Access-Control-Max-Age", "86400")
		if c.Request.Method == http.MethodOptions {
			c.AbortWithStatus(http.StatusNoContent)
			return
		}
		c.Next()
	}
}

func getClaims(c *gin.Context) *auth.Claims {
	claims, _ := c.Get(ContextKeyClaims)
	if claims == nil {
		return &auth.Claims{}
	}
	return claims.(*auth.Claims)
}

func traceID(c *gin.Context) string {
	return c.GetString(ContextKeyTraceID)
}

func bindAndValidate(c *gin.Context, obj interface{}) bool {
	if err := c.ShouldBindJSON(obj); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("invalid request: %v", err)})
		return false
	}
	return true
}