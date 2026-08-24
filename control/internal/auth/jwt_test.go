package auth

import (
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestJWTGenerateAndVerify(t *testing.T) {
	mgr := NewJWTManager()
	userID := uuid.New()
	roles := []string{"admin", "operator"}

	token, err := mgr.Generate(userID, "testuser", roles)
	if err != nil {
		t.Fatalf("generate token: %v", err)
	}
	if token == "" {
		t.Fatal("token should not be empty")
	}

	claims, err := mgr.Verify(token)
	if err != nil {
		t.Fatalf("verify token: %v", err)
	}
	if claims.UserID != userID.String() {
		t.Errorf("user_id mismatch: got %s, want %s", claims.UserID, userID.String())
	}
	if claims.Username != "testuser" {
		t.Errorf("username mismatch: got %s, want testuser", claims.Username)
	}
	if len(claims.Roles) != 2 {
		t.Errorf("roles count mismatch: got %d, want 2", len(claims.Roles))
	}
}

func TestJWTVerifyInvalidToken(t *testing.T) {
	mgr := NewJWTManager()

	if _, err := mgr.Verify("invalid.token.here"); err == nil {
		t.Fatal("should fail on invalid token")
	}

	if _, err := mgr.Verify(""); err == nil {
		t.Fatal("should fail on empty token")
	}
}

func TestJWTVerifyExpiredToken(t *testing.T) {
	mgr := &JWTManager{
		secretKey:  []byte("test-secret"),
		expiration: -1 * time.Hour,
	}
	userID := uuid.New()
	token, err := mgr.Generate(userID, "user", []string{})
	if err != nil {
		t.Fatalf("generate: %v", err)
	}

	verifyMgr := &JWTManager{
		secretKey:  []byte("test-secret"),
		expiration: 24 * time.Hour,
	}
	if _, err := verifyMgr.Verify(token); err == nil {
		t.Fatal("should fail on expired token")
	}
}

func TestJWTVerifyWrongSecret(t *testing.T) {
	mgr1 := &JWTManager{secretKey: []byte("secret1"), expiration: 24 * time.Hour}
	mgr2 := &JWTManager{secretKey: []byte("secret2"), expiration: 24 * time.Hour}

	token, _ := mgr1.Generate(uuid.New(), "user", []string{})
	if _, err := mgr2.Verify(token); err == nil {
		t.Fatal("should fail with wrong secret")
	}
}