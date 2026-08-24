package hbx

import (
	"crypto/x509"
	"encoding/pem"
	"os"
	"path/filepath"
	"testing"
)

func TestNewCA(t *testing.T) {
	ca, err := NewCA()
	if err != nil {
		t.Fatalf("NewCA failed: %v", err)
	}
	if ca.caCert == nil {
		t.Fatal("CA certificate is nil")
	}
	if ca.caKey == nil {
		t.Fatal("CA key is nil")
	}
	if !ca.caCert.IsCA {
		t.Fatal("CA certificate should have IsCA=true")
	}
	if ca.caCert.NotAfter.Before(ca.caCert.NotBefore) {
		t.Fatal("CA cert NotAfter should be after NotBefore")
	}
}

func TestCASaveLoad(t *testing.T) {
	dir := t.TempDir()

	ca, err := NewCA()
	if err != nil {
		t.Fatalf("NewCA failed: %v", err)
	}

	if err := ca.SaveToDir(dir); err != nil {
		t.Fatalf("SaveToDir failed: %v", err)
	}

	if _, err := os.Stat(filepath.Join(dir, "ca.crt")); err != nil {
		t.Fatalf("ca.crt not created: %v", err)
	}
	if _, err := os.Stat(filepath.Join(dir, "ca.key")); err != nil {
		t.Fatalf("ca.key not created: %v", err)
	}

	loaded, err := LoadFromDir(dir)
	if err != nil {
		t.Fatalf("LoadFromDir failed: %v", err)
	}
	if loaded.caCert == nil {
		t.Fatal("Loaded CA certificate is nil")
	}
	if loaded.caCert.SerialNumber.Cmp(ca.caCert.SerialNumber) != 0 {
		t.Fatal("Loaded CA serial mismatch")
	}
}

func TestSignCSRAndVerify(t *testing.T) {
	ca, err := NewCA()
	if err != nil {
		t.Fatalf("NewCA failed: %v", err)
	}

	keyPEM, csrPEM, err := GenerateAgentKeyPair("device-001")
	if err != nil {
		t.Fatalf("GenerateAgentKeyPair failed: %v", err)
	}
	if len(keyPEM) == 0 {
		t.Fatal("Key PEM is empty")
	}
	if len(csrPEM) == 0 {
		t.Fatal("CSR PEM is empty")
	}

	certPEM, err := ca.SignCSR(csrPEM, "device-001")
	if err != nil {
		t.Fatalf("SignCSR failed: %v", err)
	}
	if len(certPEM) == 0 {
		t.Fatal("Signed cert PEM is empty")
	}

	if err := ca.VerifyClientCert(certPEM); err != nil {
		t.Fatalf("VerifyClientCert failed: %v", err)
	}

	certBlock, _ := pem.Decode(certPEM)
	if certBlock == nil {
		t.Fatal("Failed to decode cert PEM")
	}
	cert, err := x509.ParseCertificate(certBlock.Bytes)
	if err != nil {
		t.Fatalf("ParseCertificate failed: %v", err)
	}
	if cert.Subject.CommonName != "device-001" {
		t.Fatalf("Expected CN=device-001, got %s", cert.Subject.CommonName)
	}
}

func TestVerifyInvalidCert(t *testing.T) {
	ca1, err := NewCA()
	if err != nil {
		t.Fatalf("NewCA failed: %v", err)
	}
	ca2, err := NewCA()
	if err != nil {
		t.Fatalf("NewCA failed: %v", err)
	}

	_, csrPEM, err := GenerateAgentKeyPair("device-002")
	if err != nil {
		t.Fatalf("GenerateAgentKeyPair failed: %v", err)
	}

	certPEM, err := ca2.SignCSR(csrPEM, "device-002")
	if err != nil {
		t.Fatalf("SignCSR failed: %v", err)
	}

	if err := ca1.VerifyClientCert(certPEM); err == nil {
		t.Fatal("Should fail to verify cert from different CA")
	}
}

func TestCACertPEM(t *testing.T) {
	ca, err := NewCA()
	if err != nil {
		t.Fatalf("NewCA failed: %v", err)
	}

	pemBytes := ca.CACertPEM()
	if len(pemBytes) == 0 {
		t.Fatal("CACertPEM returned empty")
	}

	block, _ := pem.Decode(pemBytes)
	if block == nil {
		t.Fatal("Failed to decode CA cert PEM")
	}
	if block.Type != "CERTIFICATE" {
		t.Fatalf("Expected CERTIFICATE, got %s", block.Type)
	}
}