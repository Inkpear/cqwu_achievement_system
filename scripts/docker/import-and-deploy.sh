#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
DEPLOY_SCRIPT="$SCRIPT_DIR/deploy.sh"

BUNDLE_DIR=""
MODE="full"
SERVICES=""
OVERWRITE="false"
ENV_FILE="$SCRIPT_DIR/.env"
STORAGE_ENDPOINT=""
PROJECT_NAME="${COMPOSE_PROJECT_NAME:-cqwu_achievement_system}"

usage() {
  cat <<'EOF'
Usage:
  scripts/docker/import-and-deploy.sh --bundle-dir <path> [options]

Options:
  --bundle-dir <path>         Offline bundle directory containing images.tar
  --mode <full|partial>       Deploy mode (default: full)
  --services <a,b,c>          Services for partial mode
  --overwrite                 Stop + rm selected services before deploy
  --project-name <name>       Compose project name (default: cqwu_achievement_system)
  --env-file <path>           Compose env file path (default: scripts/docker/.env)
  --storage-endpoint <url>    Override APP_STORAGE_ENDPOINT for this run
  -h, --help                  Show help

Examples:
  scripts/docker/import-and-deploy.sh --bundle-dir ./offline_bundle/v1
  scripts/docker/import-and-deploy.sh --bundle-dir ./offline_bundle/v1 --storage-endpoint http://minio.example.com:9000
  scripts/docker/import-and-deploy.sh --bundle-dir ./offline_bundle/v1 --mode partial --services backend,frontend --overwrite
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundle-dir)
      BUNDLE_DIR="$2"
      shift 2
      ;;
    --mode)
      MODE="$2"
      shift 2
      ;;
    --services)
      SERVICES="$2"
      shift 2
      ;;
    --overwrite)
      OVERWRITE="true"
      shift
      ;;
    --project-name)
      PROJECT_NAME="$2"
      shift 2
      ;;
    --env-file)
      ENV_FILE="$2"
      shift 2
      ;;
    --storage-endpoint)
      STORAGE_ENDPOINT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$BUNDLE_DIR" ]]; then
  echo "--bundle-dir is required" >&2
  usage
  exit 1
fi

TAR_FILE="$BUNDLE_DIR/images.tar"
if [[ ! -f "$TAR_FILE" ]]; then
  echo "images tar not found: $TAR_FILE" >&2
  exit 1
fi

COMPOSE_FILE="$BUNDLE_DIR/docker-compose.yml"
if [[ ! -f "$COMPOSE_FILE" ]]; then
  echo "compose file not found in bundle: $COMPOSE_FILE" >&2
  exit 1
fi

if [[ -f "$BUNDLE_DIR/images.tar.sha256" ]]; then
  echo "[step] verifying tar checksum"
  (cd "$BUNDLE_DIR" && sha256sum -c images.tar.sha256)
fi

echo "[step] loading docker images"
docker load -i "$TAR_FILE"

if [[ ! -f "$ENV_FILE" && -f "$BUNDLE_DIR/.env.example" ]]; then
  echo "[warn] env file not found: $ENV_FILE"
  echo "[warn] copy and edit this template first: $BUNDLE_DIR/.env.example"
fi

if [[ -n "$STORAGE_ENDPOINT" ]]; then
  export APP_STORAGE_ENDPOINT="$STORAGE_ENDPOINT"
fi

DEPLOY_ARGS=(--mode "$MODE" --compose-file "$COMPOSE_FILE" --project-name "$PROJECT_NAME" --env-file "$ENV_FILE" --no-build)
if [[ -n "$STORAGE_ENDPOINT" ]]; then
  DEPLOY_ARGS+=(--storage-endpoint "$STORAGE_ENDPOINT")
fi
if [[ -n "$SERVICES" ]]; then
  DEPLOY_ARGS+=(--services "$SERVICES")
fi
if [[ "$OVERWRITE" == "true" ]]; then
  DEPLOY_ARGS+=(--overwrite)
fi

echo "[step] deploying with loaded images"
"$DEPLOY_SCRIPT" "${DEPLOY_ARGS[@]}"

echo "[ok] import + deploy finished"
