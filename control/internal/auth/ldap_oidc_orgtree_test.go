package auth

import (
	"testing"
)

func TestOrgTreeAddRoot(t *testing.T) {
	tree := NewOrgTree()
	org, err := tree.AddRoot("root", "Root Org")
	if err != nil {
		t.Fatalf("AddRoot failed: %v", err)
	}
	if org.Path != "/root" {
		t.Fatalf("Expected path /root, got %s", org.Path)
	}
}

func TestOrgTreeAddChild(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")

	child, err := tree.AddChild("child1", "Child 1", "root")
	if err != nil {
		t.Fatalf("AddChild failed: %v", err)
	}
	if child.Path != "/root/child1" {
		t.Fatalf("Expected /root/child1, got %s", child.Path)
	}
	if child.ParentID == nil || *child.ParentID != "root" {
		t.Fatal("ParentID should be root")
	}
}

func TestOrgTreeAddChildParentNotFound(t *testing.T) {
	tree := NewOrgTree()
	_, err := tree.AddChild("child", "Child", "nonexistent")
	if err == nil {
		t.Fatal("Should fail with nonexistent parent")
	}
}

func TestOrgTreeGetSubtree(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("b", "B", "root")
	tree.AddChild("a1", "A1", "a")
	tree.AddChild("a2", "A2", "a")
	tree.AddChild("b1", "B1", "b")

	subtree := tree.GetSubtree("a")
	if len(subtree) != 2 {
		t.Fatalf("Expected 2 in subtree of a, got %d", len(subtree))
	}

	subtree = tree.GetSubtree("root")
	if len(subtree) != 5 {
		t.Fatalf("Expected 5 in subtree of root, got %d", len(subtree))
	}
}

func TestOrgTreeGetAncestors(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("a1", "A1", "a")

	ancestors := tree.GetAncestors("a1")
	if len(ancestors) != 2 {
		t.Fatalf("Expected 2 ancestors, got %d", len(ancestors))
	}
	if ancestors[0].ID != "root" {
		t.Fatalf("Expected root first, got %s", ancestors[0].ID)
	}
	if ancestors[1].ID != "a" {
		t.Fatalf("Expected a second, got %s", ancestors[1].ID)
	}
}

func TestOrgTreeIsAncestor(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("a1", "A1", "a")

	if !tree.IsAncestor("root", "a1") {
		t.Fatal("root should be ancestor of a1")
	}
	if !tree.IsAncestor("a", "a1") {
		t.Fatal("a should be ancestor of a1")
	}
	if tree.IsAncestor("a1", "root") {
		t.Fatal("a1 should not be ancestor of root")
	}
}

func TestOrgTreeMove(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("b", "B", "root")
	tree.AddChild("a1", "A1", "a")

	if err := tree.Move("a1", "b"); err != nil {
		t.Fatalf("Move failed: %v", err)
	}

	moved, _ := tree.Get("a1")
	if moved.Path != "/root/b/a1" {
		t.Fatalf("Expected /root/b/a1, got %s", moved.Path)
	}
}

func TestOrgTreeMoveToDescendant(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("a1", "A1", "a")

	if err := tree.Move("a", "a1"); err == nil {
		t.Fatal("Should fail to move to descendant")
	}
}

func TestOrgTreeDeleteLeaf(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")

	if err := tree.Delete("a"); err != nil {
		t.Fatalf("Delete failed: %v", err)
	}
	if _, ok := tree.Get("a"); ok {
		t.Fatal("Should be deleted")
	}
}

func TestOrgTreeDeleteWithChildren(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("a1", "A1", "a")

	if err := tree.Delete("a"); err == nil {
		t.Fatal("Should fail to delete with children")
	}
}

func TestOrgTreeGetDepth(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("a1", "A1", "a")

	if tree.GetDepth("root") != 0 {
		t.Fatal("Root depth should be 0")
	}
	if tree.GetDepth("a") != 1 {
		t.Fatal("A depth should be 1")
	}
	if tree.GetDepth("a1") != 2 {
		t.Fatal("A1 depth should be 2")
	}
}

func TestOrgTreeGetPathString(t *testing.T) {
	tree := NewOrgTree()
	tree.AddRoot("root", "Root")
	tree.AddChild("a", "A", "root")
	tree.AddChild("a1", "A1", "a")

	pathStr := tree.GetPathString("a1")
	if pathStr != "Root > A > A1" {
		t.Fatalf("Expected 'Root > A > A1', got '%s'", pathStr)
	}
}

func TestLDAPConfigRedacted(t *testing.T) {
	auth := NewLDAPAuthenticator(LDAPConfig{
		Server:       "ldap.example.com",
		BindPassword: "secret123",
	})
	c := auth.Config()
	if c.BindPassword != "***REDACTED***" {
		t.Fatal("Password should be redacted")
	}
}

func TestOIDCConfigRedacted(t *testing.T) {
	auth := NewOIDCAuthenticator(OIDCConfig{
		Issuer:       "https://keycloak.example.com",
		ClientSecret: "secret456",
	})
	c := auth.Config()
	if c.ClientSecret != "***REDACTED***" {
		t.Fatal("Secret should be redacted")
	}
}

func TestGenerateAndValidateState(t *testing.T) {
	state := GenerateState()
	if !ValidateState(state, state) {
		t.Fatal("State should validate")
	}
	if ValidateState(state, "wrong") {
		t.Fatal("Wrong state should not validate")
	}
	if ValidateState("", "") {
		t.Fatal("Empty state should not validate")
	}
}

func TestEscapeLDAPFilter(t *testing.T) {
	result := escapeLDAPFilter("user*name")
	if result != "user\\2aname" {
		t.Fatalf("Expected user\\2aname, got %s", result)
	}
}