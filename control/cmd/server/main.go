package main

import (
	"context"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/gin-gonic/gin"

	"hbx-control/internal/api"
	"hbx-control/internal/audit"
	"hbx-control/internal/auth"
	"hbx-control/internal/storage"
)

func main() {
	slog.Info("starting HBX Control Plane")

	addr := ":8080"
	if v := os.Getenv("HBX_CONTROL_ADDR"); v != "" {
		addr = v
	}

	migrationsDir := "./migrations"
	if v := os.Getenv("HBX_MIGRATIONS_DIR"); v != "" {
		migrationsDir = v
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	pgCfg := storage.DefaultPostgresConfig()
	pool, err := storage.NewPostgresPool(ctx, pgCfg)
	if err != nil {
		slog.Warn("postgres unavailable, running without DB", "error", err)
	}

	if pool != nil {
		if err := storage.RunMigrations(ctx, pool, migrationsDir); err != nil {
			slog.Error("migrations failed", "error", err)
		}
	}

	redisCfg := storage.DefaultRedisConfig()
	redisClient, err := storage.NewRedisClient(redisCfg)
	if err != nil {
		slog.Warn("redis unavailable, running without cache", "error", err)
	}

	jwtMgr := auth.NewJWTManager()
	auditLogger := audit.NewLogger(pool)
	apiServer := api.NewServer(pool, redisClient, jwtMgr, auditLogger)

	gin.SetMode(gin.ReleaseMode)
	router := gin.New()
	apiServer.RegisterRoutes(router)

	srv := &http.Server{
		Addr:         addr,
		Handler:      router,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
	}

	go func() {
		slog.Info("HBX Control Plane listening", "addr", addr)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			slog.Error("server error", "error", err)
			os.Exit(1)
		}
	}()

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	<-quit
	slog.Info("shutting down...")

	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer shutdownCancel()
	if err := srv.Shutdown(shutdownCtx); err != nil {
		slog.Error("shutdown error", "error", err)
		os.Exit(1)
	}

	if pool != nil {
		pool.Close()
	}
	if redisClient != nil {
		redisClient.Close()
	}
	slog.Info("server stopped")
}
