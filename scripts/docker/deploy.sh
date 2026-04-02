#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
DEFAULT_COMPOSE_FILE="$SCRIPT_DIR/../../docker-compose.yml"
COMPOSE_FILE="$DEFAULT_COMPOSE_FILE"
DEFAULT_ENV_FILE="$SCRIPT_DIR/.env"

MODE="full"
SERVICES_RAW=""
OVERWRITE="false"
BUILD="true"
ENV_FILE="$DEFAULT_ENV_FILE"
STORAGE_ENDPOINT=""
PROJECT_NAME="${COMPOSE_PROJECT_NAME:-cqwu_achievement_system}"

ALL_SERVICES=(postgres redis minio backend frontend)
BUILDABLE_SERVICES=(backend frontend)

needs_storage_bootstrap() {
  local svc
  for svc in "${TARGET_SERVICES[@]}"; do
    if [[ "$svc" == "minio" || "$svc" == "backend" ]]; then
      return 0
    fi
  done
  return 1
}

configure_storage_bucket_and_lifecycle() {
  if ! needs_storage_bootstrap; then
    return 0
  fi

  local storage_endpoint="${MINIO_BOOTSTRAP_ENDPOINT:-http://127.0.0.1:9000}"
  local access_key="${MINIO_ROOT_USER:-root}"
  local secret_key="${MINIO_ROOT_PASSWORD:-admin123}"
  local bucket_name="${S3_BUCKET_NAME:-achievement-bucket}"
  local temp_prefix="${S3_TEMP_PREFIX:-temp/}"
  local temp_expire_days="${S3_TEMP_EXPIRE_DAYS:-1}"

  if ! [[ "$temp_expire_days" =~ ^[0-9]+$ ]] || [[ "$temp_expire_days" -lt 1 ]]; then
    echo "[warn] invalid S3_TEMP_EXPIRE_DAYS=$temp_expire_days, fallback to 1"
    temp_expire_days="1"
  fi

  echo "[step] ensuring storage bucket and lifecycle rule"
  echo "[info] bucket=$bucket_name prefix=$temp_prefix expire_days=$temp_expire_days endpoint=$storage_endpoint"

  local attempt
  local minio_container_id
  for attempt in {1..10}; do
    minio_container_id="$("${COMPOSE_CMD[@]}" ps -q minio 2>/dev/null || true)"

    if [[ -z "$minio_container_id" ]]; then
      if [[ "$attempt" -lt 10 ]]; then
        echo "[warn] minio container not found yet, retry in 3s"
        sleep 3
      fi
      continue
    fi

    if ! docker exec "$minio_container_id" sh -ec 'command -v mc >/dev/null'; then
      echo "[warn] mc command not found in minio container, skip storage bootstrap"
      return 0
    fi

    if docker exec \
      -e STORAGE_ENDPOINT="$storage_endpoint" \
      -e ACCESS_KEY="$access_key" \
      -e SECRET_KEY="$secret_key" \
      -e BUCKET_NAME="$bucket_name" \
      -e TEMP_PREFIX="$temp_prefix" \
      -e TEMP_EXPIRE_DAYS="$temp_expire_days" \
      "$minio_container_id" sh -ec '
        mc alias set storage "$STORAGE_ENDPOINT" "$ACCESS_KEY" "$SECRET_KEY" >/dev/null
        mc mb --ignore-existing "storage/$BUCKET_NAME" >/dev/null
        mc ilm rule add --prefix "$TEMP_PREFIX" --expire-days "$TEMP_EXPIRE_DAYS" "storage/$BUCKET_NAME" >/dev/null
      '; then
      echo "[ok] storage bucket bootstrap finished"
      return 0
    fi

    if [[ "$attempt" -lt 10 ]]; then
      echo "[warn] storage bootstrap attempt $attempt failed, retry in 3s"
      sleep 3
    fi
  done

  echo "[warn] failed to bootstrap bucket/lifecycle automatically after retries"
  echo "[warn] please check minio container status/credentials and run deploy again"
}

print_storage_endpoint_hint_if_backend_selected() {
  local needs_backend="false"
  local svc
  for svc in "${TARGET_SERVICES[@]}"; do
    if [[ "$svc" == "backend" ]]; then
      needs_backend="true"
      break
    fi
  done

  if [[ "$needs_backend" != "true" ]]; then
    return 0
  fi

  local effective_endpoint="${APP_STORAGE_ENDPOINT:-http://127.0.0.1:9000}"
  echo "[info] effective APP_STORAGE_ENDPOINT=$effective_endpoint"
  echo "[info] override with --storage-endpoint or set APP_STORAGE_ENDPOINT in env/.env"
}

usage() {
  cat <<'EOF'
Usage:
  scripts/docker/deploy.sh [options]

Options:
  --mode <full|partial>       Deploy mode. Default: full
  --services <a,b,c>          Comma-separated services for partial mode
  --overwrite                 Stop + remove selected containers before deploy
  --no-build                  Skip docker compose build
  --compose-file <path>       Compose file path (default: ../../docker-compose.yml)
  --project-name <name>       Compose project name (default: cqwu_achievement_system)
  --env-file <path>           Compose env file path (default: scripts/docker/.env if exists)
  --storage-endpoint <url>    Override APP_STORAGE_ENDPOINT for this run
  -h, --help                  Show this help

Examples:
  scripts/docker/deploy.sh --mode full --overwrite
  scripts/docker/deploy.sh --mode full --storage-endpoint http://minio.example.com:9000
  scripts/docker/deploy.sh --mode partial --services backend,frontend --overwrite
  scripts/docker/deploy.sh --mode partial --services postgres,redis,minio
EOF
}

contains() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    if [[ "$item" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      MODE="$2"
      shift 2
      ;;
    --services)
      SERVICES_RAW="$2"
      shift 2
      ;;
    --overwrite)
      OVERWRITE="true"
      shift
      ;;
    --no-build)
      BUILD="false"
      shift
      ;;
    --compose-file)
      COMPOSE_FILE="$2"
      shift 2
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

if [[ ! -f "$COMPOSE_FILE" ]]; then
  echo "compose file not found: $COMPOSE_FILE" >&2
  exit 1
fi

if [[ "$MODE" != "full" && "$MODE" != "partial" ]]; then
  echo "Invalid --mode: $MODE (expected full|partial)" >&2
  exit 1
fi

declare -a TARGET_SERVICES
if [[ "$MODE" == "full" ]]; then
  TARGET_SERVICES=("${ALL_SERVICES[@]}")
else
  if [[ -z "$SERVICES_RAW" ]]; then
    echo "partial mode requires --services" >&2
    exit 1
  fi
  IFS=',' read -r -a TARGET_SERVICES <<<"$SERVICES_RAW"
fi

if [[ ${#TARGET_SERVICES[@]} -eq 0 ]]; then
  echo "No target services selected" >&2
  exit 1
fi

for svc in "${TARGET_SERVICES[@]}"; do
  if ! contains "$svc" "${ALL_SERVICES[@]}"; then
    echo "Unsupported service: $svc" >&2
    echo "Supported: ${ALL_SERVICES[*]}" >&2
    exit 1
  fi
done

COMPOSE_CMD=(docker compose --project-name "$PROJECT_NAME" -f "$COMPOSE_FILE")
if [[ -f "$ENV_FILE" ]]; then
  COMPOSE_CMD+=(--env-file "$ENV_FILE")
else
  echo "[warn] env file not found: $ENV_FILE (compose defaults will be used)"
fi

if [[ -n "$STORAGE_ENDPOINT" ]]; then
  export APP_STORAGE_ENDPOINT="$STORAGE_ENDPOINT"
fi

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck source=/dev/null
  source "$ENV_FILE"
  set +a
fi

if [[ -n "$STORAGE_ENDPOINT" ]]; then
  export APP_STORAGE_ENDPOINT="$STORAGE_ENDPOINT"
fi

print_storage_endpoint_hint_if_backend_selected

if [[ "$OVERWRITE" == "true" ]]; then
  echo "[step] overwrite enabled: stop + rm selected services"
  "${COMPOSE_CMD[@]}" stop "${TARGET_SERVICES[@]}" || true
  "${COMPOSE_CMD[@]}" rm -f "${TARGET_SERVICES[@]}" || true
fi

if [[ "$BUILD" == "true" ]]; then
  declare -a BUILD_TARGETS=()
  for svc in "${TARGET_SERVICES[@]}"; do
    if contains "$svc" "${BUILDABLE_SERVICES[@]}"; then
      BUILD_TARGETS+=("$svc")
    fi
  done

  if [[ ${#BUILD_TARGETS[@]} -gt 0 ]]; then
    echo "[step] building services: ${BUILD_TARGETS[*]}"
    "${COMPOSE_CMD[@]}" build "${BUILD_TARGETS[@]}"
  else
    echo "[step] no buildable services selected, skip build"
  fi
else
  echo "[step] --no-build enabled, skip build"
fi

if [[ "$MODE" == "full" ]]; then
  echo "[step] starting full stack"
  "${COMPOSE_CMD[@]}" up -d --remove-orphans "${TARGET_SERVICES[@]}"
else
  echo "[step] starting partial services: ${TARGET_SERVICES[*]}"
  "${COMPOSE_CMD[@]}" up -d "${TARGET_SERVICES[@]}"
fi

configure_storage_bucket_and_lifecycle

echo "[step] service status"
"${COMPOSE_CMD[@]}" ps

echo "[ok] deploy finished"
