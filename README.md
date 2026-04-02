# CQWU Achievement System - Deployment README

This document describes how to deploy the full system in this repository.

## 1. What gets deployed

The default deployment uses Docker Compose and starts 5 services:

- `postgres` (PostgreSQL 16)
- `redis` (Redis 7 with password)
- `minio` (S3-compatible object storage)
- `backend` (Rust service from root `Dockerfile`)
- `frontend` (Vue + Nginx from `cqwu_achievement_system_vue/Dockerfile`)

The frontend proxies:

- `/api/*` -> `backend:8000`
- `/health_check` -> `backend:8000/health_check`

## 2. Prerequisites

Install:

- Docker Engine (with Docker Compose v2: `docker compose`)
- Bash
- (Optional) `sha256sum` for offline bundle checksum verification

## 3. Quick online deployment

### 3.1 Prepare environment file

Create deployment env file from template:

```bash
cp scripts/docker/.env.example scripts/docker/.env
```

Edit `scripts/docker/.env` and at minimum change:

- `APP_JWT_SECRET`
- `POSTGRES_PASSWORD`
- `REDIS_PASSWORD`
- `MINIO_ROOT_PASSWORD`
- `APP_STORAGE_ENDPOINT` (must be reachable by browser/client)

Important:

- `APP_STORAGE_ENDPOINT` is used to generate presigned URLs. If it is not reachable from clients, file upload/download links will fail.
- Default values in template are for local testing and are not safe for production.
- `TZ` defaults to `CST-8` (UTC+8, POSIX format) for all services in Compose to avoid tzdata parsing issues in minimal images.
- `COMPOSE_PROJECT_NAME` should stay stable; changing it creates a different stack name and may make `--overwrite` appear ineffective.

### 3.2 Deploy full stack

```bash
scripts/docker/deploy.sh --mode full --overwrite
```

What this does:

- Optional stop+remove when `--overwrite` is provided
- Build `backend` and `frontend`
- Start all services in detached mode
- Auto create `S3_BUCKET_NAME` if not exists
- Auto add lifecycle rule: `temp/` prefix expires in 1 day by default
- Print final service status

### 3.3 Access URLs

With default ports from `.env.example`:

- Frontend: `http://localhost:8080`
- Backend: `http://localhost:8000`
- Health check: `http://localhost:8080/health_check` or `http://localhost:8000/health_check`
- MinIO API: `http://localhost:9000`
- MinIO Console: `http://localhost:9001`

## 4. Deployment script reference

Main script: `scripts/docker/deploy.sh`

### 4.1 Full deployment

```bash
scripts/docker/deploy.sh --mode full
```

### 4.2 Partial deployment

Deploy only selected services:

```bash
scripts/docker/deploy.sh --mode partial --services backend,frontend --overwrite
```

Supported service names:

- `postgres`
- `redis`
- `minio`
- `backend`
- `frontend`

### 4.3 Common options

- `--overwrite`: stop + remove selected containers before deploy
- `--no-build`: skip image build step
- `--env-file <path>`: use specific compose env file
- `--storage-endpoint <url>`: temporary override for `APP_STORAGE_ENDPOINT`

### 4.4 Automatic bucket bootstrap

`deploy.sh` automatically bootstraps object storage when deploying `minio` or `backend`:

- Create bucket from `S3_BUCKET_NAME` (if missing)
- Add lifecycle rule for temporary files

Default lifecycle behavior:

- Prefix: `temp/`
- Expiration: 1 day

Configurable env keys:

- `MINIO_BOOTSTRAP_ENDPOINT` (default: `http://127.0.0.1:9000`)
- `S3_TEMP_PREFIX` (default: `temp/`)
- `S3_TEMP_EXPIRE_DAYS` (default: `1`)

## 5. Offline deployment workflow

This repository includes scripts for air-gapped or restricted-network deployment.

### 5.1 On a machine with internet access: export bundle

```bash
scripts/docker/export-images.sh
```

Output is created under `offline_bundle/<bundle_name>/` and includes:

- `images.tar`
- `images.tar.sha256`
- `images.txt`
- `.env.example`
- `docker-compose.yml`
- `deploy.sh`
- `import-and-deploy.sh`

Custom output example:

```bash
scripts/docker/export-images.sh --output-dir ./offline_bundle --bundle-name v1_offline
```

### 5.2 Transfer bundle to target machine

Copy the whole bundle directory to the target server.

### 5.3 On target machine: import and deploy

```bash
scripts/docker/import-and-deploy.sh --bundle-dir ./offline_bundle/v1_offline --overwrite
```

This script:

- Validates checksum when `images.tar.sha256` is present
- Loads images using `docker load`
- Calls `deploy.sh` with `--no-build`
- Uses compose file from bundle: `<bundle-dir>/docker-compose.yml`

If `scripts/docker/.env` does not exist, copy template and edit:

```bash
cp ./offline_bundle/v1_offline/.env.example scripts/docker/.env
```

## 6. Runtime configuration model

Backend config loading order:

1. `configuration/base.yaml`
2. `configuration/<APP_ENVIRONMENT>.yaml` (`local` or `production`)
3. Environment variables prefixed with `APP_`

Environment variable mapping rule:

- Prefix: `APP_`
- Nested keys: `__`

Examples:

- `APP_DATABASE__HOST`
- `APP_JWT__SECRET`
- `APP_STORAGE__ENDPOINT`
- `APP_TASKS__OUTBOX_PULL_INTERVAL_MILLIS`

In Compose deployment, key runtime settings are injected through `docker-compose.yml`.

## 7. Operations and troubleshooting

### 7.1 Check status

```bash
docker compose -f docker-compose.yml --env-file scripts/docker/.env ps
```

### 7.2 View logs

```bash
docker compose -f docker-compose.yml --env-file scripts/docker/.env logs -f backend
docker compose -f docker-compose.yml --env-file scripts/docker/.env logs -f frontend
```

### 7.3 Restart one service

```bash
docker compose -f docker-compose.yml --env-file scripts/docker/.env restart backend
```

### 7.4 Rebuild app services only

```bash
scripts/docker/deploy.sh --mode partial --services backend,frontend --overwrite
```

### 7.5 Typical issues

- Frontend opens but API fails:
  - Check backend container status and backend logs.
  - Verify frontend proxy target is reachable (`backend:8000` inside Compose network).

- File upload/download URL unusable from browser:
  - `APP_STORAGE_ENDPOINT` is likely not publicly reachable from client network.
  - Set it to externally reachable MinIO/S3 endpoint.

- Offline deploy uses old app image:
  - Re-run `export-images.sh` without `--skip-prepare`, or ensure new images are built before export.

## 8. Security checklist before production

- Replace all default secrets/passwords in `scripts/docker/.env`
- Restrict exposed ports with firewall/security group
- Use HTTPS termination in front of frontend/backend endpoints
- Backup database (`postgres_data`) and object storage (`minio_data`) volumes
- Keep image tags versioned instead of always using `latest`

## 9. Useful files

- `docker-compose.yml`
- `Dockerfile`
- `cqwu_achievement_system_vue/Dockerfile`
- `cqwu_achievement_system_vue/nginx.conf`
- `scripts/docker/deploy.sh`
- `scripts/docker/export-images.sh`
- `scripts/docker/import-and-deploy.sh`
- `scripts/docker/.env.example`
- `configuration/base.yaml`
- `configuration/production.yaml`
