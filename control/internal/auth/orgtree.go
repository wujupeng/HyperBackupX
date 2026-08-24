package auth

import (
	"errors"
	"fmt"
	"strings"
	"sync"
)

// Organization 组织结构
type Organization struct {
	ID       string
	Name     string
	Path     string
	ParentID *string
}

// OrgTree 组织结构树（物化路径实现）
type OrgTree struct {
	mu   sync.RWMutex
	orgs map[string]*Organization
}

// NewOrgTree 创建组织结构树
func NewOrgTree() *OrgTree {
	return &OrgTree{
		orgs: make(map[string]*Organization),
	}
}

// AddRoot 添加根组织
func (t *OrgTree) AddRoot(id, name string) (*Organization, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	if _, exists := t.orgs[id]; exists {
		return nil, errors.New("organization already exists")
	}

	org := &Organization{
		ID:   id,
		Name: name,
		Path: "/" + id,
	}
	t.orgs[id] = org
	return org, nil
}

// AddChild 添加子组织
func (t *OrgTree) AddChild(id, name, parentID string) (*Organization, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	if _, exists := t.orgs[id]; exists {
		return nil, errors.New("organization already exists")
	}
	parent, exists := t.orgs[parentID]
	if !exists {
		return nil, errors.New("parent organization not found")
	}

	org := &Organization{
		ID:       id,
		Name:     name,
		Path:     parent.Path + "/" + id,
		ParentID: &parentID,
	}
	t.orgs[id] = org
	return org, nil
}

// Get 获取组织
func (t *OrgTree) Get(id string) (*Organization, bool) {
	t.mu.RLock()
	defer t.mu.RUnlock()

	org, ok := t.orgs[id]
	if !ok {
		return nil, false
	}
	return org, true
}

// GetSubtree 获取子树（所有后代组织）
// 使用物化路径前缀匹配：path LIKE '/parent/.../%'
func (t *OrgTree) GetSubtree(id string) []*Organization {
	t.mu.RLock()
	defer t.mu.RUnlock()

	org, ok := t.orgs[id]
	if !ok {
		return nil
	}

	prefix := org.Path + "/"
	var result []*Organization
	for _, o := range t.orgs {
		if strings.HasPrefix(o.Path, prefix) {
			result = append(result, o)
		}
	}
	return result
}

// GetAncestors 获取祖先链
func (t *OrgTree) GetAncestors(id string) []*Organization {
	t.mu.RLock()
	defer t.mu.RUnlock()

	org, ok := t.orgs[id]
	if !ok {
		return nil
	}

	pathParts := strings.Split(strings.Trim(org.Path, "/"), "/")
	var ancestors []*Organization
	for i := 0; i < len(pathParts)-1; i++ {
		ancestorID := pathParts[i]
		if a, ok := t.orgs[ancestorID]; ok {
			ancestors = append(ancestors, a)
		}
	}
	return ancestors
}

// IsAncestor 检查 ancestorID 是否是 id 的祖先
func (t *OrgTree) IsAncestor(ancestorID, id string) bool {
	t.mu.RLock()
	defer t.mu.RUnlock()

	ancestor, ok := t.orgs[ancestorID]
	if !ok {
		return false
	}
	org, ok := t.orgs[id]
	if !ok {
		return false
	}
	return strings.HasPrefix(org.Path, ancestor.Path+"/")
}

// IsDescendant 检查 descendantID 是否是 id 的后代
func (t *OrgTree) IsDescendant(descendantID, id string) bool {
	return t.IsAncestor(id, descendantID)
}

// List 列出所有组织
func (t *OrgTree) List() []*Organization {
	t.mu.RLock()
	defer t.mu.RUnlock()

	result := make([]*Organization, 0, len(t.orgs))
	for _, o := range t.orgs {
		result = append(result, o)
	}
	return result
}

// Move 移动组织到新父节点
func (t *OrgTree) Move(id, newParentID string) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	org, ok := t.orgs[id]
	if !ok {
		return errors.New("organization not found")
	}
	if id == newParentID {
		return errors.New("cannot move to self")
	}

	newParent, ok := t.orgs[newParentID]
	if !ok {
		return errors.New("new parent not found")
	}

	if strings.HasPrefix(newParent.Path, org.Path+"/") {
		return errors.New("cannot move to descendant")
	}

	oldPrefix := org.Path
	newPath := newParent.Path + "/" + id
	org.Path = newPath
	org.ParentID = &newParentID

	for _, o := range t.orgs {
		if strings.HasPrefix(o.Path, oldPrefix+"/") {
			o.Path = newPath + o.Path[len(oldPrefix):]
		}
	}

	return nil
}

// Delete 删除组织（仅叶子节点可删除）
func (t *OrgTree) Delete(id string) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	org, ok := t.orgs[id]
	if !ok {
		return errors.New("organization not found")
	}

	prefix := org.Path + "/"
	for _, o := range t.orgs {
		if strings.HasPrefix(o.Path, prefix) {
			return errors.New("cannot delete organization with children")
		}
	}

	delete(t.orgs, id)
	return nil
}

// GetDepth 获取组织深度（根=0）
func (t *OrgTree) GetDepth(id string) int {
	t.mu.RLock()
	defer t.mu.RUnlock()

	org, ok := t.orgs[id]
	if !ok {
		return -1
	}
	return strings.Count(strings.Trim(org.Path, "/"), "/")
}

// GetPathString 获取路径的可读字符串
func (t *OrgTree) GetPathString(id string) string {
	t.mu.RLock()
	defer t.mu.RUnlock()

	org, ok := t.orgs[id]
	if !ok {
		return ""
	}

	pathParts := strings.Split(strings.Trim(org.Path, "/"), "/")
	var names []string
	for _, partID := range pathParts {
		if o, ok := t.orgs[partID]; ok {
			names = append(names, o.Name)
		}
	}
	return strings.Join(names, " > ")
}

// String 返回树的字符串表示
func (t *OrgTree) String() string {
	t.mu.RLock()
	defer t.mu.RUnlock()

	var sb strings.Builder
	for _, o := range t.orgs {
		depth := strings.Count(strings.Trim(o.Path, "/"), "/")
		indent := strings.Repeat("  ", depth)
		sb.WriteString(fmt.Sprintf("%s%s (%s)\n", indent, o.Name, o.ID))
	}
	return sb.String()
}