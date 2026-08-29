package middleware

import (
	"net/http"

	"hbx-control/internal/auth"

	"github.com/gin-gonic/gin"
)

var passwordChangeWhitelist = map[string]bool{
	"/api/v1/auth/change-password": true,
	"/api/v1/auth/logout":          true,
	"/api/v1/auth/refresh":         true,
	"/healthz":                     true,
}

func PasswordChangeMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		claimsVal, exists := c.Get("hbx.claims")
		if !exists {
			c.Next()
			return
		}

		claims, ok := claimsVal.(*auth.Claims)
		if !ok {
			c.Next()
			return
		}

		if !claims.MustChangePassword {
			c.Next()
			return
		}

		path := c.Request.URL.Path
		if passwordChangeWhitelist[path] {
			c.Next()
			return
		}

		c.Header("X-HBX-Require-Password-Change", "true")
		c.JSON(http.StatusForbidden, gin.H{"error": "password change required"})
		c.Abort()
	}
}
