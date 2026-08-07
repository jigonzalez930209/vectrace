#!/usr/bin/env bash
# Local e2e harness for Vectrace multi-compositor capture probes.
# Not invoked by CI.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
E2E="$ROOT/e2e"
MATRIX="$E2E/matrix.yaml"
REPORTS="$E2E/reports"
GOLDENS="$E2E/goldens"
PROBE_BIN="${PROBE_BIN:-$ROOT/target/debug/vectrace-e2e-probe}"

die() { echo "error: $*" >&2; exit 1; }

need_matrix() {
  [[ -f "$MATRIX" ]] || die "missing $MATRIX"
}

ensure_probe() {
  if [[ ! -x "$PROBE_BIN" ]]; then
    echo "Building vectrace-e2e-probe..."
    (cd "$ROOT" && cargo build --bin vectrace-e2e-probe)
  fi
  [[ -x "$PROBE_BIN" ]] || die "probe binary missing at $PROBE_BIN"
}

# Minimal YAML field extractors (matrix is intentionally simple).
# Usage: yaml_scenarios -> prints scenario ids
yaml_scenario_ids() {
  awk '
    /^[[:space:]]*- id:[[:space:]]*/ {
      sub(/^[[:space:]]*- id:[[:space:]]*/, "")
      gsub(/["\047]/, "")
      print
    }
  ' "$MATRIX"
}

# Extract a block for scenario id into a temp-friendly stream of "key: value" lines
# (top-level keys under the scenario, flattened one level for expect.*).
scenario_block() {
  local id="$1"
  awk -v want="$id" '
    BEGIN { in_sc=0; indent=0 }
    /^[[:space:]]*- id:[[:space:]]*/ {
      line=$0
      sub(/^[[:space:]]*- id:[[:space:]]*/, "", line)
      gsub(/["\047]/, "", line)
      if (line == want) { in_sc=1; next }
      else if (in_sc) { exit }
    }
    in_sc {
      if ($0 ~ /^[[:space:]]*- id:[[:space:]]*/) exit
      print
    }
  ' "$MATRIX"
}

sc_get() {
  local id="$1" key="$2"
  scenario_block "$id" | awk -v key="$key" '
    BEGIN { re="^[[:space:]]*" key ":[[:space:]]*" }
    $0 ~ re {
      sub(re, "")
      gsub(/^[[:space:]]+|[[:space:]]+$/, "")
      gsub(/["\047]/, "")
      print
      exit
    }
  '
}

sc_expect_min_size() {
  local id="$1"
  # expect min_size: [W, H]
  scenario_block "$id" | awk '
    /min_size:[[:space:]]*\[/ {
      line=$0
      sub(/.*\[/, "", line)
      sub(/\].*/, "", line)
      gsub(/[[:space:]]/, "", line)
      split(line, a, ",")
      print a[1] "x" a[2]
      exit
    }
  '
}

sc_expect_path() {
  local id="$1"
  sc_get "$id" "capture_path"
}

sc_expect_path_any() {
  local id="$1"
  # capture_path_any: [a, b] -> a|b
  scenario_block "$id" | awk '
    /capture_path_any:[[:space:]]*\[/ {
      line=$0
      sub(/.*\[/, "", line)
      sub(/\].*/, "", line)
      gsub(/["\047[:space:]]/, "", line)
      gsub(/,/, "|", line)
      print line
      exit
    }
  '
}

sc_flash_forbidden() {
  local id="$1"
  local v
  v="$(scenario_block "$id" | awk '
    /flash_forbidden:[[:space:]]*/ {
      sub(/.*flash_forbidden:[[:space:]]*/, "")
      gsub(/[[:space:]]/, "")
      print
      exit
    }
  ')"
  [[ "$v" == "true" ]]
}

sc_expect_overlay() {
  local id="$1"
  sc_get "$id" "overlay"
}

cmd_list() {
  need_matrix
  printf "%-24s %-10s %-8s %s\n" "ID" "RUNNER" "STATUS" "DE"
  while IFS= read -r id; do
    [[ -z "$id" ]] && continue
    printf "%-24s %-10s %-8s %s\n" \
      "$id" \
      "$(sc_get "$id" "runner")" \
      "$(sc_get "$id" "status")" \
      "$(sc_get "$id" "de")"
  done < <(yaml_scenario_ids)
}

run_probe_args_for() {
  local id="$1"
  local -a args=(--scenario "$id")
  local path path_any overlay minsize

  path="$(sc_expect_path "$id" || true)"
  path_any="$(sc_expect_path_any "$id" || true)"
  overlay="$(sc_expect_overlay "$id" || true)"
  minsize="$(sc_expect_min_size "$id" || true)"

  if [[ -n "${path_any:-}" ]]; then
    args+=(--expect-path-any "$path_any")
  elif [[ -n "${path:-}" ]]; then
    args+=(--expect-path "$path")
  fi
  if [[ -n "${overlay:-}" ]]; then
    args+=(--expect-overlay "$overlay")
  fi
  if [[ -n "${minsize:-}" ]]; then
    args+=(--min-size "$minsize")
  fi
  if sc_flash_forbidden "$id"; then
    args+=(--flash-forbidden)
  fi
  printf '%s\n' "${args[@]}"
}

cmd_run() {
  local id="${1:-}"
  [[ -n "$id" ]] || { echo "usage: harness.sh run <scenario_id>" >&2; return 1; }
  need_matrix
  ensure_probe

  local status runner nest_script
  status="$(sc_get "$id" "status")"
  runner="$(sc_get "$id" "runner")"
  nest_script="$(sc_get "$id" "nest_script")"

  if [[ "$status" == "blocked" ]]; then
    echo "SKIP blocked: $id ($(sc_get "$id" "blocked_reason"))"
    return 2
  fi

  local ts out
  ts="$(date +%s)"
  out="$REPORTS/$id/$ts"
  mkdir -p "$out"

  mapfile -t probe_args < <(run_probe_args_for "$id")
  probe_args+=(--out-dir "$out")

  # Persist args for nest scripts (avoids fragile word-splitting).
  : >"$out/probe.args"
  for a in "${probe_args[@]}"; do
    printf '%s\0' "$a" >>"$out/probe.args"
  done

  echo "=== run $id (runner=$runner status=$status) ==="
  echo "out: $out"

  local rc=0
  set +e
  if [[ "$runner" == "nested" ]]; then
    [[ -n "$nest_script" ]] || { echo "scenario $id missing nest_script" >&2; return 1; }
    local script="$E2E/$nest_script"
    [[ -f "$script" ]] || { echo "nest script not found: $script" >&2; return 1; }
    chmod +x "$script" 2>/dev/null || true
    PROBE_BIN="$PROBE_BIN" OUT_DIR="$out" E2E_ROOT="$E2E" ROOT="$ROOT" \
      bash "$script"
    rc=$?
  else
    "$PROBE_BIN" "${probe_args[@]}"
    rc=$?
  fi

  echo "$out" > "$REPORTS/$id/latest"
  if [[ $rc -eq 0 ]]; then
    echo "OK $id -> $out"
  elif [[ $rc -eq 2 ]]; then
    echo "SKIP $id"
  else
    echo "FAIL $id (exit $rc)"
  fi
  return "$rc"
}

cmd_run_all() {
  need_matrix
  ensure_probe
  local failed=0 skipped=0 ok=0
  # Disable -e for the loop: scenario skips return 2.
  set +e
  while IFS= read -r id; do
    [[ -z "$id" ]] && continue
    local status runner rc
    status="$(sc_get "$id" "status")"
    runner="$(sc_get "$id" "runner")"
    if [[ "$status" == "blocked" ]]; then
      echo "SKIP blocked $id"
      skipped=$((skipped + 1))
      continue
    fi
    if [[ "$runner" == "attach" && "${E2E_INCLUDE_ATTACH:-0}" != "1" ]]; then
      echo "SKIP attach_only $id (set E2E_INCLUDE_ATTACH=1 to include)"
      skipped=$((skipped + 1))
      continue
    fi
    cmd_run "$id"
    rc=$?
    if [[ $rc -eq 0 ]]; then
      ok=$((ok + 1))
    elif [[ $rc -eq 2 ]]; then
      skipped=$((skipped + 1))
    else
      failed=$((failed + 1))
    fi
  done < <(yaml_scenario_ids)
  set -e
  echo "=== summary: ok=$ok skipped=$skipped failed=$failed ==="
  [[ "$failed" -eq 0 ]] || exit 1
}

json_field() {
  local file="$1" field="$2"
  # naive JSON string/number extractor for our flat reports
  awk -v f="$field" '
    $0 ~ "\"" f "\"" {
      line=$0
      if (match(line, /:[[:space:]]*"[^"]*"/)) {
        s=substr(line, RSTART, RLENGTH)
        sub(/^:[[:space:]]*"/, "", s)
        sub(/"$/, "", s)
        print s
        exit
      }
      if (match(line, /:[[:space:]]*[0-9]+/)) {
        s=substr(line, RSTART, RLENGTH)
        sub(/^:[[:space:]]*/, "", s)
        sub(/,$/, "", s)
        print s
        exit
      }
      if (match(line, /:[[:space:]]*(true|false)/)) {
        s=substr(line, RSTART, RLENGTH)
        sub(/^:[[:space:]]*/, "", s)
        sub(/,$/, "", s)
        print s
        exit
      }
    }
  ' "$file"
}

cmd_compare() {
  local a="${1:-}" b="${2:-}"
  [[ -n "$a" && -n "$b" ]] || die "usage: harness.sh compare <dir_a> <dir_b>"
  local ra rb
  ra="$a/report.json"
  rb="$b/report.json"
  [[ -f "$ra" ]] || die "missing $ra"
  [[ -f "$rb" ]] || die "missing $rb"

  echo "Comparing:"
  echo "  A=$ra"
  echo "  B=$rb"

  local pa pb wa wb ha hb oa ob
  pa="$(json_field "$ra" capture_path)"
  pb="$(json_field "$rb" capture_path)"
  wa="$(json_field "$ra" width)"; ha="$(json_field "$ra" height)"
  wb="$(json_field "$rb" width)"; hb="$(json_field "$rb" height)"
  oa="$(json_field "$ra" overlay_hint)"
  ob="$(json_field "$rb" overlay_hint)"
  local oka okb
  oka="$(json_field "$ra" ok)"
  okb="$(json_field "$rb" ok)"

  printf "path:    %s vs %s\n" "$pa" "$pb"
  printf "size:    %sx%s vs %sx%s\n" "$wa" "$ha" "$wb" "$hb"
  printf "overlay: %s vs %s\n" "$oa" "$ob"
  printf "ok:      %s vs %s\n" "$oka" "$okb"

  local fail=0
  [[ "$oka" == "true" && "$okb" == "true" ]] || { echo "FAIL: one or both reports not ok"; fail=1; }
  # Cross-env: do not require identical path/size; only both healthy.
  if [[ $fail -eq 0 ]]; then
    echo "COMPARE OK (both reports healthy; pixel identity not required)"
  else
    exit 1
  fi
}

cmd_run_docker() {
  need_matrix
  ensure_probe
  local failed=0 skipped=0 ok=0 softfail=0
  echo "=== docker profile: required nested X11 + best-effort Wayland ==="
  set +e
  while IFS= read -r id; do
    [[ -z "$id" ]] && continue
    local docker_flag status rc
    docker_flag="$(sc_get "$id" "docker")"
    status="$(sc_get "$id" "status")"
    if [[ "$status" == "blocked" ]]; then
      continue
    fi
    if [[ "$docker_flag" != "true" && "$docker_flag" != "best_effort" ]]; then
      continue
    fi
    cmd_run "$id"
    rc=$?
    if [[ $rc -eq 0 ]]; then
      ok=$((ok + 1))
    elif [[ $rc -eq 2 ]]; then
      skipped=$((skipped + 1))
    elif [[ "$docker_flag" == "best_effort" ]]; then
      echo "SOFT-FAIL (best_effort) $id"
      softfail=$((softfail + 1))
    else
      failed=$((failed + 1))
    fi
  done < <(yaml_scenario_ids)
  set -e
  echo "=== docker summary: ok=$ok skipped=$skipped softfail=$softfail failed=$failed ==="
  [[ "$failed" -eq 0 ]] || exit 1
}

cmd_bless() {
  local id="${1:-}"
  [[ -n "$id" ]] || die "usage: harness.sh bless <scenario_id>"
  local latest="$REPORTS/$id/latest"
  [[ -f "$latest" ]] || die "no latest report for $id; run it first"
  local dir
  dir="$(cat "$latest")"
  local src="$dir/capture.png"
  [[ -f "$src" ]] || die "missing $src"
  mkdir -p "$GOLDENS"
  cp -f "$src" "$GOLDENS/$id.png"
  echo "Blessed golden: $GOLDENS/$id.png"
}

cmd_help() {
  cat <<EOF
Vectrace local e2e harness (not used by CI)

Usage:
  $0 list
  $0 run <scenario_id>
  $0 run-all                 # nested + supported; skip attach unless E2E_INCLUDE_ATTACH=1
  $0 run-docker              # docker-safe nested matrix (see e2e/docker/)
  $0 compare <dir_a> <dir_b>
  $0 bless <scenario_id>

Env:
  PROBE_BIN=path             # default: target/debug/vectrace-e2e-probe
  E2E_INCLUDE_ATTACH=1       # include attach_only scenarios in run-all

Docker (nested X11/Weston/Sway without installing compositors on the host):
  ./e2e/docker/docker-run.sh build
  ./e2e/docker/docker-run.sh run
EOF
}

main() {
  local cmd="${1:-help}"
  shift || true
  case "$cmd" in
    list) cmd_list "$@" ;;
    run) cmd_run "$@"; exit $? ;;
    run-all) cmd_run_all "$@" ;;
    run-docker) cmd_run_docker "$@" ;;
    compare) cmd_compare "$@" ;;
    bless) cmd_bless "$@" ;;
    help|-h|--help) cmd_help ;;
    *) die "unknown command: $cmd (try help)" ;;
  esac
}

main "$@"
