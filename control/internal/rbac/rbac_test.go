package rbac

import "testing"

func TestCheckWildcardPermission(t *testing.T) {
	perms := []string{"*"}
	if !Check(perms, PermDevicesRead) {
		t.Error("wildcard * should match any permission")
	}
	if !Check(perms, PermUsersAll) {
		t.Error("wildcard * should match any permission")
	}
}

func TestCheckExactPermission(t *testing.T) {
	perms := []string{"devices:read"}
	if !Check(perms, PermDevicesRead) {
		t.Error("exact match should pass")
	}
	if Check(perms, PermDevicesWrite) {
		t.Error("non-matching permission should fail")
	}
}

func TestCheckPrefixWildcard(t *testing.T) {
	perms := []string{"jobs:*"}
	if !Check(perms, PermJobsAll) {
		t.Error("jobs:* should match jobs:*")
	}
	if !Check(perms, PermJobsTrigger) {
		t.Error("jobs:* should match jobs:trigger")
	}
	if Check(perms, PermDevicesRead) {
		t.Error("jobs:* should not match devices:read")
	}
}

func TestCheckAny(t *testing.T) {
	perms := []string{"devices:read", "logs:read"}
	if !CheckAny(perms, PermDevicesRead, PermDevicesWrite) {
		t.Error("should pass with devices:read")
	}
	if !CheckAny(perms, PermLogsRead, PermAuditRead) {
		t.Error("should pass with logs:read")
	}
	if CheckAny(perms, PermUsersAll, PermRolesAll) {
		t.Error("should fail with no matching permissions")
	}
}

func TestCheckEmptyPermissions(t *testing.T) {
	if Check([]string{}, PermDevicesRead) {
		t.Error("empty permissions should not match anything")
	}
	if Check(nil, PermDevicesRead) {
		t.Error("nil permissions should not match anything")
	}
}

func TestBuiltinRolePermissions(t *testing.T) {
	adminPerms := []string{"*"}
	operatorPerms := []string{"devices:read", "jobs:*", "versions:read", "restores:*", "verify:*", "monitoring:read", "logs:read"}
	auditorPerms := []string{"audit:read", "logs:read"}

	if !Check(adminPerms, PermUsersAll) {
		t.Error("admin should have all permissions")
	}
	if !Check(operatorPerms, PermJobsTrigger) {
		t.Error("operator should trigger jobs")
	}
	if Check(operatorPerms, PermUsersAll) {
		t.Error("operator should not manage users")
	}
	if !Check(auditorPerms, PermAuditRead) {
		t.Error("auditor should read audit logs")
	}
	if Check(auditorPerms, PermUsersAll) {
		t.Error("auditor should not manage users")
	}
}

func TestBadouPermissions(t *testing.T) {
	storageAdminPerms := []string{"badou:read", "badou:write", "badou:admin"}

	if !Check(storageAdminPerms, PermBadouRead) {
		t.Error("storage admin should have badou:read")
	}
	if !Check(storageAdminPerms, PermBadouWrite) {
		t.Error("storage admin should have badou:write")
	}
	if !Check(storageAdminPerms, PermBadouAdmin) {
		t.Error("storage admin should have badou:admin")
	}

	operatorPerms := []string{"badou:read", "badou:write"}
	if !Check(operatorPerms, PermBadouRead) {
		t.Error("operator should have badou:read")
	}
	if !Check(operatorPerms, PermBadouWrite) {
		t.Error("operator should have badou:write")
	}
	if Check(operatorPerms, PermBadouAdmin) {
		t.Error("operator should not have badou:admin")
	}

	viewerPerms := []string{"badou:read"}
	if !Check(viewerPerms, PermBadouRead) {
		t.Error("viewer should have badou:read")
	}
	if Check(viewerPerms, PermBadouWrite) {
		t.Error("viewer should not have badou:write")
	}
}

func TestBadouWildcardAdmin(t *testing.T) {
	adminPerms := []string{"*"}
	if !Check(adminPerms, PermBadouRead) {
		t.Error("admin * should match badou:read")
	}
	if !Check(adminPerms, PermBadouWrite) {
		t.Error("admin * should match badou:write")
	}
	if !Check(adminPerms, PermBadouAdmin) {
		t.Error("admin * should match badou:admin")
	}
}