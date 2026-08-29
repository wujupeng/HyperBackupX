package api

import (
	"github.com/gin-gonic/gin"

	"hbx-control/internal/badou/handler"
	"hbx-control/internal/badou/repository"
	"hbx-control/internal/badou/service"
	"hbx-control/internal/rbac"
)

func (s *Server) initBadouHandler() *handler.Handler {
	repo := repository.NewBadouRepo(s.pool)
	svc := service.NewService(repo, service.NewStubClient())
	auditFwd := service.NewAuditForwarder(s.auditLogger)
	return handler.NewHandler(svc, auditFwd)
}

func (s *Server) registerBadouRoutes(authed *gin.RouterGroup) {
	h := s.initBadouHandler()

	badouGroup := authed.Group("/badou")
	{
		badouGroup.GET("/repositories", s.RBAC(rbac.PermBadouRead), h.ListRepositories)
		badouGroup.POST("/repositories", s.RBAC(rbac.PermBadouAdmin), h.CreateRepository)
		badouGroup.GET("/repositories/:id", s.RBAC(rbac.PermBadouRead), h.GetRepository)
		badouGroup.PUT("/repositories/:id", s.RBAC(rbac.PermBadouAdmin), h.UpdateRepository)
		badouGroup.DELETE("/repositories/:id", s.RBAC(rbac.PermBadouAdmin), h.DeleteRepository)
		badouGroup.POST("/repositories/:id/immutable", s.RBAC(rbac.PermBadouAdmin), h.SetImmutable)
		badouGroup.GET("/repositories/:id/versions", s.RBAC(rbac.PermBadouRead), h.ListVersions)
		badouGroup.GET("/repositories/:id/versions/:vid", s.RBAC(rbac.PermBadouRead), h.GetVersion)
		badouGroup.DELETE("/repositories/:id/versions/:vid", s.RBAC(rbac.PermBadouWrite), h.DeleteVersion)
		badouGroup.POST("/repositories/:id/verify", s.RBAC(rbac.PermBadouAdmin), h.VerifyRepository)
		badouGroup.POST("/repositories/:id/gc", s.RBAC(rbac.PermBadouAdmin), h.TriggerGC)
		badouGroup.GET("/repositories/:id/gc/report", s.RBAC(rbac.PermBadouRead), h.GetGCReport)

		badouGroup.GET("/cluster/nodes", s.RBAC(rbac.PermBadouRead), h.ListNodes)
		badouGroup.POST("/cluster/nodes", s.RBAC(rbac.PermBadouAdmin), h.AddNode)
		badouGroup.DELETE("/cluster/nodes/:id", s.RBAC(rbac.PermBadouAdmin), h.RemoveNode)
		badouGroup.GET("/cluster/health", s.RBAC(rbac.PermBadouRead), h.ClusterHealth)
		badouGroup.POST("/cluster/capacity", s.RBAC(rbac.PermBadouAdmin), h.ExpandCapacity)
	}
}