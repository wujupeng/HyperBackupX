package auth

import (
	"context"
	"crypto/tls"
	"errors"
	"fmt"
	"net"
	"strings"
	"time"
)

// LDAPConfig AD/LDAP 认证配置
type LDAPConfig struct {
	Server       string
	Port         int
	UseTLS       bool
	BaseDN       string
	BindDN       string
	BindPassword string
	UserFilter   string
	GroupFilter  string
	Timeout      time.Duration
}

// LDAPAuthenticator AD/LDAP 认证器
type LDAPAuthenticator struct {
	config LDAPConfig
}

// NewLDAPAuthenticator 创建 LDAP 认证器
func NewLDAPAuthenticator(config LDAPConfig) *LDAPAuthenticator {
	if config.Timeout == 0 {
		config.Timeout = 10 * time.Second
	}
	if config.Port == 0 {
		if config.UseTLS {
			config.Port = 636
		} else {
			config.Port = 389
		}
	}
	if config.UserFilter == "" {
		config.UserFilter = "(sAMAccountName=%s)"
	}
	return &LDAPAuthenticator{config: config}
}

// LDAPUser LDAP 用户信息
type LDAPUser struct {
	DN         string
	Username   string
	Email      string
	DisplayName string
	Groups     []string
}

// Authenticate 认证用户
// 使用 LDAP 简单绑定：先绑定服务账号搜索用户 DN，再用用户 DN+密码绑定验证
func (a *LDAPAuthenticator) Authenticate(ctx context.Context, username, password string) (*LDAPUser, error) {
	if username == "" || password == "" {
		return nil, errors.New("username and password required")
	}

	conn, err := a.dial(ctx)
	if err != nil {
		return nil, fmt.Errorf("LDAP connect failed: %w", err)
	}
	defer conn.Close()

	if err := a.bind(conn, a.config.BindDN, a.config.BindPassword); err != nil {
		return nil, fmt.Errorf("service bind failed: %w", err)
	}

	userDN, err := a.searchUserDN(conn, username)
	if err != nil {
		return nil, err
	}

	if err := a.bind(conn, userDN, password); err != nil {
		return nil, errors.New("invalid credentials")
	}

	return &LDAPUser{
		DN:       userDN,
		Username: username,
	}, nil
}

func (a *LDAPAuthenticator) dial(ctx context.Context) (net.Conn, error) {
	addr := fmt.Sprintf("%s:%d", a.config.Server, a.config.Port)

	dialer := &net.Dialer{Timeout: a.config.Timeout}

	if a.config.UseTLS {
		tlsConfig := &tls.Config{ServerName: a.config.Server}
		return tls.DialWithDialer(dialer, "tcp", addr, tlsConfig)
	}

	return dialer.DialContext(ctx, "tcp", addr)
}

func (a *LDAPAuthenticator) bind(conn net.Conn, dn, password string) error {
	packet := encodeSimpleBindRequest(dn, password)
	if _, err := conn.Write(packet); err != nil {
		return err
	}

	resp := make([]byte, 1024)
	n, err := conn.Read(resp)
	if err != nil {
		return err
	}

	return parseBindResponse(resp[:n])
}

func (a *LDAPAuthenticator) searchUserDN(conn net.Conn, username string) (string, error) {
	filter := fmt.Sprintf(a.config.UserFilter, escapeLDAPFilter(username))
	packet := encodeSearchRequest(a.config.BaseDN, filter)
	if _, err := conn.Write(packet); err != nil {
		return "", err
	}

	resp := make([]byte, 4096)
	n, err := conn.Read(resp)
	if err != nil {
		return "", err
	}

	return parseSearchResultDN(resp[:n])
}

func escapeLDAPFilter(s string) string {
	s = strings.ReplaceAll(s, "\\", "\\5c")
	s = strings.ReplaceAll(s, "*", "\\2a")
	s = strings.ReplaceAll(s, "(", "\\28")
	s = strings.ReplaceAll(s, ")", "\\29")
	s = strings.ReplaceAll(s, "\x00", "\\00")
	return s
}

func encodeSimpleBindRequest(dn, password string) []byte {
	dnBytes := []byte(dn)
	pwBytes := []byte(password)

	seqLen := 2 + len(dnBytes) + 2 + len(pwBytes) + 2
	result := []byte{0x30, byte(seqLen), 0x02, 0x01, 0x01}
	result = append(result, 0x60, byte(len(dnBytes)+len(pwBytes)+4))
	result = append(result, 0x04, byte(len(dnBytes)))
	result = append(result, dnBytes...)
	result = append(result, 0x04, byte(len(pwBytes)))
	result = append(result, pwBytes...)
	return result
}

func parseBindResponse(data []byte) error {
	if len(data) < 7 {
		return errors.New("invalid bind response")
	}
	if len(data) >= 10 {
		resultCode := data[9]
		if resultCode != 0 {
			return fmt.Errorf("LDAP bind failed: code %d", resultCode)
		}
	}
	return nil
}

func encodeSearchRequest(baseDN, filter string) []byte {
	return encodeSimpleBindRequest(baseDN, filter)
}

func parseSearchResultDN(data []byte) (string, error) {
	if len(data) < 10 {
		return "", errors.New("no search results")
	}
	return "", errors.New("LDAP search not fully implemented (requires go-ldap)")
}

// IsAvailable 检查 LDAP 服务器是否可达
func (a *LDAPAuthenticator) IsAvailable(ctx context.Context) bool {
	conn, err := a.dial(ctx)
	if err != nil {
		return false
	}
	conn.Close()
	return true
}

// Config 返回配置（脱敏后）
func (a *LDAPAuthenticator) Config() LDAPConfig {
	c := a.config
	c.BindPassword = "***REDACTED***"
	return c
}