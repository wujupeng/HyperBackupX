.PHONY: all build test lint fmt clean build-agent build-control build-web build-tray test-agent test-control test-web

all: build

build: build-agent build-control build-web

build-agent:
	cargo build --workspace

build-control:
	cd control && go build ./...

build-web:
	cd web && npm run build

build-tray:
	cd tray && cmake -B build -S . && cmake --build build

test: test-agent test-control test-web

test-agent:
	cargo test --workspace

test-control:
	cd control && go test ./...

test-web:
	cd web && npm run test

lint:
	cargo clippy --workspace -- -D warnings
	cd control && go vet ./...
	cd web && npm run lint

fmt:
	cargo fmt --all
	cd control && go fmt ./...
	cd web && npm run format

clean:
	cargo clean
	cd web && rm -rf dist node_modules
	cd tray && rm -rf build

dev-up:
	docker-compose up -d

dev-down:
	docker-compose down