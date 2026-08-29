package api

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
)

func SetSecureCookie(c *gin.Context, name, value string, maxAge time.Duration) {
	cookie := &http.Cookie{
		Name:     name,
		Value:    value,
		MaxAge:   int(maxAge.Seconds()),
		Path:     "/",
		Secure:   true,
		HttpOnly: true,
		SameSite: http.SameSiteStrictMode,
	}
	c.Writer.Header().Add("Set-Cookie", cookie.String())
}

func ClearSecureCookie(c *gin.Context, name string) {
	cookie := &http.Cookie{
		Name:     name,
		Value:    "",
		MaxAge:   -1,
		Path:     "/",
		Secure:   true,
		HttpOnly: true,
		SameSite: http.SameSiteStrictMode,
	}
	c.Writer.Header().Add("Set-Cookie", cookie.String())
}

func GetCookieToken(c *gin.Context) string {
	token, err := c.Cookie("hbx_token")
	if err != nil {
		return ""
	}
	return token
}
