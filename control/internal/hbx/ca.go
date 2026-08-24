package hbx

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"errors"
	"math/big"
	"os"
	"path/filepath"
	"time"
)

// CA 是 HyperBackup X 内部证书颁发机构
type CA struct {
	caCert    *x509.Certificate
	caKey     *ecdsa.PrivateKey
	caCertPEM []byte
	caKeyPEM  []byte
}

// NewCA 创建新的自签名 CA
func NewCA() (*CA, error) {
	caKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, err
	}

	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return nil, err
	}

	template := &x509.Certificate{
		SerialNumber: serial,
		Subject: pkix.Name{
			Organization: []string{"HyperBackup X"},
			CommonName:   "HBX Internal CA",
		},
		NotBefore:             time.Now(),
		NotAfter:              time.Now().AddDate(10, 0, 0),
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		BasicConstraintsValid: true,
		IsCA:                  true,
		MaxPathLen:            1,
	}

	caCertDER, err := x509.CreateCertificate(rand.Reader, template, template, &caKey.PublicKey, caKey)
	if err != nil {
		return nil, err
	}

	caCert, err := x509.ParseCertificate(caCertDER)
	if err != nil {
		return nil, err
	}

	caCertPEM := pem.EncodeToMemory(&pem.Block{
		Type:  "CERTIFICATE",
		Bytes: caCertDER,
	})

	caKeyDER, err := x509.MarshalECPrivateKey(caKey)
	if err != nil {
		return nil, err
	}

	caKeyPEM := pem.EncodeToMemory(&pem.Block{
		Type:  "EC PRIVATE KEY",
		Bytes: caKeyDER,
	})

	return &CA{
		caCert:    caCert,
		caKey:     caKey,
		caCertPEM: caCertPEM,
		caKeyPEM:  caKeyPEM,
	}, nil
}

// SaveToDir 将 CA 证书和密钥保存到目录
func (ca *CA) SaveToDir(dir string) error {
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(dir, "ca.crt"), ca.caCertPEM, 0644); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(dir, "ca.key"), ca.caKeyPEM, 0600); err != nil {
		return err
	}
	return nil
}

// LoadFromDir 从目录加载 CA
func LoadFromDir(dir string) (*CA, error) {
	certPEM, err := os.ReadFile(filepath.Join(dir, "ca.crt"))
	if err != nil {
		return nil, err
	}
	keyPEM, err := os.ReadFile(filepath.Join(dir, "ca.key"))
	if err != nil {
		return nil, err
	}

	certBlock, _ := pem.Decode(certPEM)
	if certBlock == nil {
		return nil, errors.New("failed to decode CA certificate PEM")
	}
	caCert, err := x509.ParseCertificate(certBlock.Bytes)
	if err != nil {
		return nil, err
	}

	keyBlock, _ := pem.Decode(keyPEM)
	if keyBlock == nil {
		return nil, errors.New("failed to decode CA key PEM")
	}
	caKey, err := x509.ParseECPrivateKey(keyBlock.Bytes)
	if err != nil {
		return nil, err
	}

	return &CA{
		caCert:    caCert,
		caKey:     caKey,
		caCertPEM: certPEM,
		caKeyPEM:  keyPEM,
	}, nil
}

// CACertPEM 返回 CA 证书的 PEM 编码
func (ca *CA) CACertPEM() []byte {
	return ca.caCertPEM
}

// SignCSR 签署 Agent 的 CSR，返回签发的证书 PEM
func (ca *CA) SignCSR(csrPEM []byte, deviceID string) ([]byte, error) {
	csrBlock, _ := pem.Decode(csrPEM)
	if csrBlock == nil {
		return nil, errors.New("failed to decode CSR PEM")
	}

	csr, err := x509.ParseCertificateRequest(csrBlock.Bytes)
	if err != nil {
		return nil, err
	}

	if err := csr.CheckSignature(); err != nil {
		return nil, errors.New("CSR signature invalid")
	}

	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return nil, err
	}

	template := &x509.Certificate{
		SerialNumber: serial,
		Subject: pkix.Name{
			Organization: []string{"HyperBackup X"},
			CommonName:   deviceID,
		},
		NotBefore:   time.Now(),
		NotAfter:    time.Now().AddDate(2, 0, 0),
		KeyUsage:    x509.KeyUsageDigitalSignature | x509.KeyUsageKeyEncipherment,
		ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth, x509.ExtKeyUsageServerAuth},
		DNSNames:    csr.DNSNames,
		IPAddresses: csr.IPAddresses,
	}

	certDER, err := x509.CreateCertificate(rand.Reader, template, ca.caCert, csr.PublicKey, ca.caKey)
	if err != nil {
		return nil, err
	}

	certPEM := pem.EncodeToMemory(&pem.Block{
		Type:  "CERTIFICATE",
		Bytes: certDER,
	})

	return certPEM, nil
}

// VerifyClientCert 验证客户端证书是否由本 CA 签发
func (ca *CA) VerifyClientCert(certPEM []byte) error {
	certBlock, _ := pem.Decode(certPEM)
	if certBlock == nil {
		return errors.New("failed to decode certificate PEM")
	}

	cert, err := x509.ParseCertificate(certBlock.Bytes)
	if err != nil {
		return err
	}

	roots := x509.NewCertPool()
	roots.AddCert(ca.caCert)

	opts := x509.VerifyOptions{
		Roots:     roots,
		KeyUsages: []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth},
	}

	_, err = cert.Verify(opts)
	return err
}

// GenerateAgentKeyPair 生成 Agent 密钥对和 CSR
// 返回: 私钥 PEM, CSR PEM, error
func GenerateAgentKeyPair(deviceID string) ([]byte, []byte, error) {
	agentKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, nil, err
	}

	keyDER, err := x509.MarshalECPrivateKey(agentKey)
	if err != nil {
		return nil, nil, err
	}

	keyPEM := pem.EncodeToMemory(&pem.Block{
		Type:  "EC PRIVATE KEY",
		Bytes: keyDER,
	})

	csrTemplate := &x509.CertificateRequest{
		Subject: pkix.Name{
			Organization: []string{"HyperBackup X"},
			CommonName:   deviceID,
		},
	}

	csrDER, err := x509.CreateCertificateRequest(rand.Reader, csrTemplate, agentKey)
	if err != nil {
		return nil, nil, err
	}

	csrPEM := pem.EncodeToMemory(&pem.Block{
		Type:  "CERTIFICATE REQUEST",
		Bytes: csrDER,
	})

	return keyPEM, csrPEM, nil
}