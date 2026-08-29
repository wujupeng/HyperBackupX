package auth

import (
	"context"
	"fmt"
	"regexp"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"
	"golang.org/x/crypto/bcrypt"
)

var (
	ErrInvalidOldPassword = fmt.Errorf("invalid old password")
	ErrPasswordTooWeak    = fmt.Errorf("new password does not meet strength requirements")
)

type ChangePasswordRequest struct {
	OldPassword string `json:"old_password"`
	NewPassword string `json:"new_password"`
}

type ChangePasswordService struct {
	pool *pgxpool.Pool
	jwt  *JWTManager
}

func NewChangePasswordService(pool *pgxpool.Pool, jwt *JWTManager) *ChangePasswordService {
	return &ChangePasswordService{pool: pool, jwt: jwt}
}

func (s *ChangePasswordService) ChangePassword(ctx context.Context, userID, oldPassword, newPassword string) error {
	var currentHash string
	err := s.pool.QueryRow(ctx,
		`SELECT password_hash FROM users WHERE user_id = $1`, userID).Scan(&currentHash)
	if err != nil {
		return fmt.Errorf("query user: %w", err)
	}

	if err := bcrypt.CompareHashAndPassword([]byte(currentHash), []byte(oldPassword)); err != nil {
		return ErrInvalidOldPassword
	}

	if !validatePasswordStrength(newPassword) {
		return ErrPasswordTooWeak
	}

	newHash, err := bcrypt.GenerateFromPassword([]byte(newPassword), bcrypt.DefaultCost)
	if err != nil {
		return fmt.Errorf("hash password: %w", err)
	}

	_, err = s.pool.Exec(ctx,
		`UPDATE users SET password_hash = $1, must_change_password = false, password_changed_at = NOW() WHERE user_id = $2`,
		string(newHash), userID)
	if err != nil {
		return fmt.Errorf("update password: %w", err)
	}

	return nil
}

func validatePasswordStrength(password string) bool {
	if len(password) < 16 {
		return false
	}
	hasUpper := regexp.MustCompile(`[A-Z]`).MatchString(password)
	hasLower := regexp.MustCompile(`[a-z]`).MatchString(password)
	hasDigit := regexp.MustCompile(`[0-9]`).MatchString(password)
	hasSymbol := regexp.MustCompile(`[!@#$%^&*()_+\-=\[\]{};':"\\,.<>\/?]`).MatchString(password)
	return hasUpper && hasLower && hasDigit && hasSymbol
}

func (s *ChangePasswordService) NeedsPasswordChange(ctx context.Context, userID string) (bool, error) {
	var mustChange bool
	err := s.pool.QueryRow(ctx,
		`SELECT must_change_password FROM users WHERE user_id = $1`, userID).Scan(&mustChange)
	if err != nil {
		return false, err
	}
	return mustChange, nil
}

type RefreshService struct {
	jwt *JWTManager
}

func NewRefreshService(jwt *JWTManager) *RefreshService {
	return &RefreshService{jwt: jwt}
}

func (s *RefreshService) RefreshToken(oldToken string) (string, *Claims, error) {
	claims, err := s.jwt.Verify(oldToken)
	if err != nil {
		return "", nil, fmt.Errorf("verify token: %w", err)
	}

	newToken, err := s.jwt.Generate(
		parseUUID(claims.UserID),
		claims.Username,
		claims.Roles,
	)
	if err != nil {
		return "", nil, fmt.Errorf("generate token: %w", err)
	}

	return newToken, claims, nil
}

func parseUUID(s string) uuid.UUID {
	id, _ := uuid.Parse(s)
	return id
}
