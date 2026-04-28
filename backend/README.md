# Crucible Backend

> Production-ready API server for the Crucible smart contract testing platform, built with Rust, Axum, PostgreSQL, and Redis.

---

## Architecture

```
┌──────────────┐     ┌──────────────────────┐     ┌────────────────┐
│   Clients    │────▶│   Axum HTTP Server    │────▶│  PostgreSQL 16 │
│  (port 8080) │     │                      │     │  (port 5432)   │
└──────────────┘     │  Middleware Stack:    │     └────────────────┘
                     │  ├─ CORS             │
                     │  ├─ Tracing          │     ┌────────────────┐
                     │  ├─ Compression      │────▶│   Redis 7      │
                     │  └─ Request ID       │     │  (port 6379)   │
                     └──────────────────────┘     └────────────────┘
```

## Services

| Service       | Image                 | Port  | Purpose                         |
|---------------|-----------------------|-------|----------------------------------|
| `app`         | Custom (Dockerfile)   | 8080  | Rust/Axum HTTP API server       |
| `postgres`    | `postgres:16-alpine`  | 5432  | Primary database (SQLx)         |
| `redis`       | `redis:7-alpine`      | 6379  | Caching & job queues            |
| `pgadmin`     | `dpage/pgadmin4`      | 5050  | DB admin UI (dev-tools profile) |
| `redis-commander` | `rediscommander/redis-commander` | 8081 | Redis admin UI (dev-tools profile) |

## Quick Start

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) ≥ 24.0
- [Docker Compose](https://docs.docker.com/compose/install/) ≥ 2.20
- [Rust](https://rustup.rs/) ≥ 1.78 (for local development)

### 1. Clone and configure

```bash
cd backend
cp .env.example .env
# Edit .env to set secrets for production
```

### 2. Start services

```bash
# Start all core services (app, postgres, redis)
docker compose up -d

# Start with admin tools (pgAdmin + Redis Commander)
docker compose --profile dev-tools up -d

# Rebuild after code changes
docker compose up -d --build
```

### 3. Verify

```bash
# Check service health
docker compose ps

# Test the health endpoint
curl http://localhost:8080/health

# Expected response:
# {"status":"ok","version":"0.1.0","database":"healthy","redis":"healthy"}

# Test the API status endpoint
curl http://localhost:8080/api/v1/status
```

### 4. View logs

```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f app
docker compose logs -f postgres
docker compose logs -f redis
```

### 5. Stop services

```bash
# Stop (preserves data volumes)
docker compose down

# Stop and remove all data
docker compose down -v
```

## Local Development (without Docker)

For faster iteration, run Postgres and Redis in Docker but the Rust app natively:

```bash
# Start only infrastructure services
docker compose up -d postgres redis

# Run the Rust app locally
export DATABASE_URL=postgres://crucible:crucible_secret@localhost:5432/crucible_db
export REDIS_URL=redis://:crucible_redis_secret@localhost:6379/0
cargo run
```

### Running Tests

```bash
# Unit tests (no external services needed)
cargo test

# With all features
cargo test --all-features

# Integration tests (requires running postgres + redis)
cargo test -- --ignored
```

## Project Structure

```
backend/
├── docker-compose.yml      # Docker Compose service orchestration
├── Dockerfile              # Multi-stage build for the Rust binary
├── Cargo.toml              # Rust dependencies and build configuration
├── .env.example            # Environment variable template
├── .dockerignore           # Files excluded from Docker build context
├── README.md               # This file
├── migrations/             # SQLx database migrations
│   └── .keep
├── scripts/
│   └── init-db.sql         # Database initialization (schema + seeds)
└── src/
    ├── main.rs             # Application entry point, router, health checks
    └── error.rs            # Custom error types with HTTP status mapping
```

## Environment Variables

| Variable                  | Default                     | Description                              |
|---------------------------|-----------------------------|------------------------------------------|
| `APP_ENV`                 | `development`               | Environment (`development`/`production`) |
| `APP_PORT`                | `8080`                      | HTTP server port                         |
| `RUST_LOG`                | `crucible_backend=debug`    | Log level filter                         |
| `DATABASE_URL`            | *(composed from parts)*     | Full PostgreSQL connection string        |
| `POSTGRES_USER`           | `crucible`                  | PostgreSQL username                      |
| `POSTGRES_PASSWORD`       | `crucible_secret`           | PostgreSQL password                      |
| `POSTGRES_DB`             | `crucible_db`               | PostgreSQL database name                 |
| `DATABASE_MAX_CONNECTIONS`| `10`                        | Max pool connections                     |
| `DATABASE_MIN_CONNECTIONS`| `2`                         | Min pool connections                     |
| `REDIS_URL`               | *(composed from parts)*     | Full Redis connection string             |
| `REDIS_PASSWORD`          | `crucible_redis_secret`     | Redis authentication password            |
| `REDIS_POOL_SIZE`         | `5`                         | Redis connection pool size               |
| `JWT_SECRET`              | *(dev default)*             | JWT signing secret                       |
| `CORS_ALLOWED_ORIGINS`    | `localhost:3000,5173`       | Comma-separated allowed origins          |

## Docker Compose Features

### Health Checks

All services include Docker health checks:

- **PostgreSQL**: `pg_isready` command verifying database connectivity
- **Redis**: `redis-cli PING` command verifying cache availability
- **App**: HTTP `GET /health` checking both downstream dependencies

The `app` service uses `depends_on` with `condition: service_healthy` to ensure infrastructure is ready before starting.

### Resource Limits

Each service has memory and CPU limits configured via `deploy.resources`:

| Service    | Memory Limit | CPU Limit | Memory Reserve | CPU Reserve |
|------------|-------------|-----------|----------------|-------------|
| `app`      | 512 MB      | 2.0       | 128 MB         | 0.5         |
| `postgres` | 512 MB      | 1.0       | 128 MB         | 0.25        |
| `redis`    | 256 MB      | 0.5       | 64 MB          | 0.1         |

### Persistent Volumes

Named volumes ensure data survives container restarts:

- `crucible-postgres-data` — PostgreSQL data directory
- `crucible-redis-data` — Redis append-only file and snapshots
- `crucible-pgadmin-data` — pgAdmin configuration

### Networking

All services communicate over the `crucible-network` bridge network with a dedicated subnet (`172.28.0.0/16`), isolating traffic from other Docker workloads.

### Logging

JSON file logging with rotation:
- App: 50 MB max file, 5 files retained
- Infrastructure: 10 MB max file, 3 files retained

## Database Schema

The init script (`scripts/init-db.sql`) creates:

| Table         | Purpose                                      |
|---------------|----------------------------------------------|
| `contracts`   | Deployed smart contract metadata             |
| `test_runs`   | Test execution results per contract          |
| `test_cases`  | Individual test results within a run         |
| `jobs`        | Background job queue tracking                |

Extensions enabled: `uuid-ossp`, `pgcrypto`, `citext`

## API Endpoints

| Method | Path             | Description                    |
|--------|------------------|--------------------------------|
| `GET`  | `/health`        | Health check (DB + Redis)      |
| `GET`  | `/api/v1/status` | API status and version info    |

## Production Deployment

For production, update the following:

1. **Change all passwords** in `.env` — never use defaults
2. **Set `APP_ENV=production`**
3. **Set `RUST_LOG=crucible_backend=info,tower_http=info`** — reduce log verbosity
4. **Set a strong `JWT_SECRET`** — at least 64 characters
5. **Restrict `CORS_ALLOWED_ORIGINS`** — to your frontend domain(s)
6. **Consider external managed databases** — for PostgreSQL and Redis at scale
7. **Add TLS termination** — via a reverse proxy (nginx, Caddy, or cloud LB)

## License

MIT — see [LICENSE](../LICENSE) for details.
