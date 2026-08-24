package rbac

import (
	"strings"
)

type Permission string

const (
	PermAll          Permission = "*"
	PermDevicesRead  Permission = "devices:read"
	PermDevicesWrite Permission = "devices:write"
	PermPoliciesAll  Permission = "policies:*"
	PermJobsAll      Permission = "jobs:*"
	PermJobsTrigger  Permission = "jobs:trigger"
	PermVersionsRead Permission = "versions:read"
	PermRestoresAll  Permission = "restores:*"
	PermVerifyAll    Permission = "verify:*"
	PermMonitorRead  Permission = "monitoring:read"
	PermAlertsAll    Permission = "alerts:*"
	PermLogsRead     Permission = "logs:read"
	PermAuditRead    Permission = "audit:read"
	PermUsersAll     Permission = "users:*"
	PermRolesAll     Permission = "roles:*"
	PermOrgsAll      Permission = "organizations:*"
	PermUpgradeAll   Permission = "upgrade:*"
)

func Check(permissions []string, required Permission) bool {
	for _, p := range permissions {
		if p == string(PermAll) {
			return true
		}
		if p == string(required) {
			return true
		}
		if strings.HasSuffix(p, ":*") {
			prefix := strings.TrimSuffix(p, "*")
			if strings.HasPrefix(string(required), prefix) {
				return true
			}
		}
	}
	return false
}

func CheckAny(permissions []string, required ...Permission) bool {
	for _, r := range required {
		if Check(permissions, r) {
			return true
		}
	}
	return false
}