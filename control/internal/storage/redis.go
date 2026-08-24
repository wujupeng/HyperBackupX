package storage

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/redis/go-redis/v9"
)

type RedisConfig struct {
	Addr     string
	Password string
	DB       int
}

func DefaultRedisConfig() RedisConfig {
	cfg := RedisConfig{
		Addr:     "localhost:6379",
		Password: "",
		DB:       0,
	}
	if v := os.Getenv("HBX_REDIS_ADDR"); v != "" {
		cfg.Addr = v
	}
	if v := os.Getenv("HBX_REDIS_PASSWORD"); v != "" {
		cfg.Password = v
	}
	return cfg
}

func NewRedisClient(cfg RedisConfig) (*redis.Client, error) {
	client := redis.NewClient(&redis.Options{
		Addr:         cfg.Addr,
		Password:     cfg.Password,
		DB:           cfg.DB,
		DialTimeout:  5 * time.Second,
		ReadTimeout:  3 * time.Second,
		WriteTimeout: 3 * time.Second,
		PoolSize:     20,
	})

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := client.Ping(ctx).Err(); err != nil {
		client.Close()
		return nil, fmt.Errorf("ping redis: %w", err)
	}

	slog.Info("redis connected", "addr", cfg.Addr)
	return client, nil
}