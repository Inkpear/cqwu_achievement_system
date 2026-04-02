#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
ROOT_DIR="$SCRIPT_DIR/../.."
COMPOSE_FILE="$ROOT_DIR/docker-compose.yml"
ENV_FILE="$SCRIPT_DIR/.env"

OUTPUT_DIR="$ROOT_DIR/offline_bundle"
BUNDLE_NAME="cqwu_achievement_bundle_$(date +%Y%m%d_%H%M%S)"
SKIP_PREPARE="false"
STORAGE_ENDPOINT=""

usage() {
  cat <<'EOF'
Usage:
  scripts/docker/export-images.sh [options]

Options:
  --output-dir <path>         Output directory for bundle
  --bundle-name <name>        Bundle folder name
  --skip-prepare              Skip pull/build before save
  --storage-endpoint <url>    Override APP_STORAGE_ENDPOINT for this run
  -h, --help                  Show help

Examples:
  scripts/docker/export-images.sh
  scripts/docker/export-images.sh --storage-endpoint http://minio.example.com:9000
  scripts/docker/export-images.sh --output-dir ./release --bundle-name v1_offline
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --bundle-name)
      BUNDLE_NAME="$2"
      shift 2
      ;;
    --skip-prepare)
      SKIP_PREPARE="true"
      shift
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

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck source=/dev/null
  source "$ENV_FILE"
  set +a
fi

if [[ -n "$STORAGE_ENDPOINT" ]]; then
  export APP_STORAGE_ENDPOINT="$STORAGE_ENDPOINT"
fi

BACKEND_IMAGE="${BACKEND_IMAGE:-cqwu-achievement/backend:latest}"
FRONTEND_IMAGE="${FRONTEND_IMAGE:-cqwu-achievement/frontend:latest}"
POSTGRES_IMAGE="postgres:16-alpine"
REDIS_IMAGE="redis:7-alpine"
MINIO_IMAGE="minio/minio:RELEASE.2025-02-18T16-25-55Z"

IMAGES=(
  "$BACKEND_IMAGE"
  "$FRONTEND_IMAGE"
  "$POSTGRES_IMAGE"
  "$REDIS_IMAGE"
  "$MINIO_IMAGE"
)

COMPOSE_CMD=(docker compose -f "$COMPOSE_FILE")
if [[ -f "$ENV_FILE" ]]; then
  COMPOSE_CMD+=(--env-file "$ENV_FILE")
fi

if [[ "$SKIP_PREPARE" != "true" ]]; then
  echo "[step] pull base dependency images"
  "${COMPOSE_CMD[@]}" pull postgres redis minio

  echo "[step] build app images"
  "${COMPOSE_CMD[@]}" build backend frontend
else
  echo "[step] --skip-prepare enabled, skip pull/build"
fi

BUNDLE_DIR="$OUTPUT_DIR/$BUNDLE_NAME"
mkdir -p "$BUNDLE_DIR"

echo "[step] save images to tar"
docker save -o "$BUNDLE_DIR/images.tar" "${IMAGES[@]}"

printf '%s\n' "${IMAGES[@]}" > "$BUNDLE_DIR/images.txt"
(
  cd "$BUNDLE_DIR"
  sha256sum images.tar > images.tar.sha256
)
cp "$ROOT_DIR/scripts/docker/.env.example" "$BUNDLE_DIR/.env.example"
cp "$COMPOSE_FILE" "$BUNDLE_DIR/docker-compose.yml"
cp "$SCRIPT_DIR/deploy.sh" "$BUNDLE_DIR/deploy.sh"
cp "$SCRIPT_DIR/import-and-deploy.sh" "$BUNDLE_DIR/import-and-deploy.sh"

echo "[ok] export finished"
echo "Bundle: $BUNDLE_DIR"
echo "Images:"
cat "$BUNDLE_DIR/images.txt"
