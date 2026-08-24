package auth

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// OIDCConfig OIDC 认证配置
type OIDCConfig struct {
	Issuer       string
	ClientID     string
	ClientSecret string
	RedirectURI  string
	Scopes       []string
	Timeout      time.Duration
}

// OIDCAuthenticator OIDC 认证器
type OIDCAuthenticator struct {
	config   OIDCConfig
	client   *http.Client
	metadata *OIDCDiscoveryMetadata
}

// OIDCDiscoveryMetadata OIDC 发现元数据
type OIDCDiscoveryMetadata struct {
	Issuer                 string `json:"issuer"`
	AuthorizationEndpoint  string `json:"authorization_endpoint"`
	TokenEndpoint          string `json:"token_endpoint"`
	UserInfoEndpoint       string `json:"userinfo_endpoint"`
	JWKSURI                string `json:"jwks_uri"`
	EndSessionEndpoint     string `json:"end_session_endpoint"`
}

// OIDCTokenResponse OIDC 令牌响应
type OIDCTokenResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	IDToken      string `json:"id_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int    `json:"expires_in"`
}

// OIDCUserInfo OIDC 用户信息
type OIDCUserInfo struct {
	Sub          string `json:"sub"`
	Email        string `json:"email"`
	Name         string `json:"name"`
	DisplayName  string `json:"preferred_username"`
	Groups       []string `json:"groups"`
}

// NewOIDCAuthenticator 创建 OIDC 认证器
func NewOIDCAuthenticator(config OIDCConfig) *OIDCAuthenticator {
	if config.Timeout == 0 {
		config.Timeout = 15 * time.Second
	}
	if len(config.Scopes) == 0 {
		config.Scopes = []string{"openid", "profile", "email"}
	}
	return &OIDCAuthenticator{
		config: config,
		client: &http.Client{Timeout: config.Timeout},
	}
}

// Discover 获取 OIDC 发现元数据
func (a *OIDCAuthenticator) Discover(ctx context.Context) error {
	wellKnown := strings.TrimSuffix(a.config.Issuer, "/") + "/.well-known/openid-configuration"

	req, err := http.NewRequestWithContext(ctx, "GET", wellKnown, nil)
	if err != nil {
		return err
	}

	resp, err := a.client.Do(req)
	if err != nil {
		return fmt.Errorf("OIDC discovery failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("OIDC discovery returned %d", resp.StatusCode)
	}

	var metadata OIDCDiscoveryMetadata
	if err := json.NewDecoder(resp.Body).Decode(&metadata); err != nil {
		return fmt.Errorf("OIDC discovery parse failed: %w", err)
	}

	a.metadata = &metadata
	return nil
}

// GetAuthorizationURL 生成授权码流跳转 URL
func (a *OIDCAuthenticator) GetAuthorizationURL(state string) (string, error) {
	if a.metadata == nil {
		return "", errors.New("OIDC not discovered, call Discover first")
	}

	u, err := url.Parse(a.metadata.AuthorizationEndpoint)
	if err != nil {
		return "", err
	}

	q := u.Query()
	q.Set("response_type", "code")
	q.Set("client_id", a.config.ClientID)
	q.Set("redirect_uri", a.config.RedirectURI)
	q.Set("state", state)
	q.Set("scope", strings.Join(a.config.Scopes, " "))
	u.RawQuery = q.Encode()

	return u.String(), nil
}

// ExchangeCode 用授权码交换令牌
func (a *OIDCAuthenticator) ExchangeCode(ctx context.Context, code string) (*OIDCTokenResponse, error) {
	if a.metadata == nil {
		return nil, errors.New("OIDC not discovered")
	}

	data := url.Values{}
	data.Set("grant_type", "authorization_code")
	data.Set("code", code)
	data.Set("client_id", a.config.ClientID)
	data.Set("client_secret", a.config.ClientSecret)
	data.Set("redirect_uri", a.config.RedirectURI)

	req, err := http.NewRequestWithContext(ctx, "POST", a.metadata.TokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	resp, err := a.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("token exchange failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("token exchange returned %d: %s", resp.StatusCode, string(body))
	}

	var tokenResp OIDCTokenResponse
	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return nil, fmt.Errorf("token response parse failed: %w", err)
	}

	return &tokenResp, nil
}

// GetUserInfo 用 access_token 获取用户信息
func (a *OIDCAuthenticator) GetUserInfo(ctx context.Context, accessToken string) (*OIDCUserInfo, error) {
	if a.metadata == nil {
		return nil, errors.New("OIDC not discovered")
	}

	req, err := http.NewRequestWithContext(ctx, "GET", a.metadata.UserInfoEndpoint, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Authorization", "Bearer "+accessToken)

	resp, err := a.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("userinfo request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("userinfo returned %d", resp.StatusCode)
	}

	var userInfo OIDCUserInfo
	if err := json.NewDecoder(resp.Body).Decode(&userInfo); err != nil {
		return nil, fmt.Errorf("userinfo parse failed: %w", err)
	}

	return &userInfo, nil
}

// RefreshToken 刷新令牌
func (a *OIDCAuthenticator) RefreshToken(ctx context.Context, refreshToken string) (*OIDCTokenResponse, error) {
	if a.metadata == nil {
		return nil, errors.New("OIDC not discovered")
	}

	data := url.Values{}
	data.Set("grant_type", "refresh_token")
	data.Set("refresh_token", refreshToken)
	data.Set("client_id", a.config.ClientID)
	data.Set("client_secret", a.config.ClientSecret)

	req, err := http.NewRequestWithContext(ctx, "POST", a.metadata.TokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	resp, err := a.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("token refresh failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("token refresh returned %d", resp.StatusCode)
	}

	var tokenResp OIDCTokenResponse
	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return nil, err
	}

	return &tokenResp, nil
}

// GenerateState 生成 OIDC state 参数
func GenerateState() string {
	return fmt.Sprintf("oidc-%d", time.Now().UnixNano())
}

// ValidateState 验证 state 参数
func ValidateState(expected, actual string) bool {
	return expected == actual && expected != ""
}

// Config 返回配置（脱敏后）
func (a *OIDCAuthenticator) Config() OIDCConfig {
	c := a.config
	c.ClientSecret = "***REDACTED***"
	return c
}