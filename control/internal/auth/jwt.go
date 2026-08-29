package auth

import (
	"errors"
	"fmt"

	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
)

type Claims struct {
	UserID   string   `json:"user_id"`
	Username string   `json:"username"`
	Roles    []string `json:"roles"`
	jwt.RegisteredClaims
}

type JWTManager struct {
	secretKey  []byte
	expiration time.Duration
}

var ErrJWTSecretNotConfigured = fmt.Errorf("JWT Secret 未配置，请通过 SecretLoader 注入")

func NewJWTManager(secret []byte) (*JWTManager, error) {
	if len(secret) < 32 {
		return nil, ErrJWTSecretNotConfigured
	}
	return &JWTManager{
		secretKey:  secret,
		expiration: 24 * time.Hour,
	}, nil
}

func (m *JWTManager) Generate(userID uuid.UUID, username string, roles []string) (string, error) {
	claims := &Claims{
		UserID:   userID.String(),
		Username: username,
		Roles:    roles,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(time.Now().Add(m.expiration)),
			IssuedAt:  jwt.NewNumericDate(time.Now()),
			Subject:   userID.String(),
		},
	}
	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	return token.SignedString(m.secretKey)
}

func (m *JWTManager) Verify(tokenStr string) (*Claims, error) {
	token, err := jwt.ParseWithClaims(tokenStr, &Claims{}, func(t *jwt.Token) (interface{}, error) {
		if _, ok := t.Method.(*jwt.SigningMethodHMAC); !ok {
			return nil, fmt.Errorf("unexpected signing method: %v", t.Header["alg"])
		}
		return m.secretKey, nil
	})
	if err != nil {
		return nil, fmt.Errorf("parse token: %w", err)
	}
	claims, ok := token.Claims.(*Claims)
	if !ok || !token.Valid {
		return nil, errors.New("invalid token claims")
	}
	return claims, nil
}
