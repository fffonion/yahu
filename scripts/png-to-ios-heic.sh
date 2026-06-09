#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Convert a PNG (or any imageflow-readable input) to iOS-compatible HEIC.

Usage:
  png-to-ios-heic.sh [-q QUALITY] [-d DESCRIPTION] INPUT OUTPUT
  png-to-ios-heic.sh INPUT OUTPUT [QUALITY]

Options:
  -q, --quality QUALITY   HEIC lossy quality, 0-100. Default: 82
  -d, --description TEXT  Store TEXT as the HEIC primary image user description
  -h, --help              Show this help

Workflow:
  1. imageflow_tool decodes the input and writes an opaque 8-bit RGB PNG
     intermediate (PNG24, white matte by default).
  2. libheif/heif-enc encodes that PNG as HEVC HEIC with x265, 4:2:0 chroma,
     no alpha, and an sRGB-ish nclx color profile. This produces files that
     heif-info reports as MIME image/heic, main brand heic, item type hvc1.

Environment:
  MATTE_HEX=RRGGBB        Matte color for transparent input pixels. Default: FFFFFF
  HEIF_ENC=/path/heif-enc Override heif-enc path
  HEIF_INFO=/path/heif-info Override heif-info path
  HEIF_PLUGIN_DIR=/path   Override libheif plugin dir, used for x265 plugin
  NO_BOOTSTRAP_HEIF=1     Do not apt-download user-local libheif tools if missing

If heif-enc/x265 is not installed system-wide, the script can bootstrap
libheif-examples + libheif-plugin-x265 into ~/.hermes/cache/tools/libheif-local
without sudo, using apt-get download + dpkg-deb -x.
USAGE
}

quality=82
description=""
args=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -q|--quality)
      [[ $# -ge 2 ]] || { echo "error: $1 requires a value" >&2; exit 2; }
      quality="$2"
      shift 2
      ;;
    -d|--description|--pitm-description)
      [[ $# -ge 2 ]] || { echo "error: $1 requires a value" >&2; exit 2; }
      description="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      while [[ $# -gt 0 ]]; do args+=("$1"); shift; done
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      args+=("$1")
      shift
      ;;
  esac
done

if [[ ${#args[@]} -eq 3 ]]; then
  quality="${args[2]}"
elif [[ ${#args[@]} -ne 2 ]]; then
  usage >&2
  exit 2
fi

input="${args[0]}"
output="${args[1]}"
matte_hex="${MATTE_HEX:-FFFFFF}"

if [[ ! "$quality" =~ ^[0-9]+$ ]] || (( quality < 0 || quality > 100 )); then
  echo "error: quality must be an integer in 0..100, got: $quality" >&2
  exit 2
fi
if [[ ! "$matte_hex" =~ ^[0-9A-Fa-f]{6}$ ]]; then
  echo "error: MATTE_HEX must be exactly 6 hex chars, got: $matte_hex" >&2
  exit 2
fi
if [[ ! -f "$input" ]]; then
  echo "error: input file not found: $input" >&2
  exit 1
fi
if ! command -v imageflow_tool >/dev/null 2>&1; then
  echo "error: imageflow_tool is required but not in PATH" >&2
  exit 1
fi

hermes_home="${HERMES_HOME:-$HOME/.hermes}"
local_root="$hermes_home/cache/tools/libheif-local"
local_bin="$local_root/usr/bin"
local_plugin_dir="$local_root/usr/lib/x86_64-linux-gnu/libheif/plugins"

heif_enc_args=()
description_args=()
if [[ -n "$description" ]]; then
  description_args+=(--pitm-description "$description")
fi

# Capture output rather than piping under pipefail, so missing plugins don't
# abort the caller before we can try the user-local bootstrap path.
has_x265_encoder() {
  local enc="$1"
  local plugin_dir="${2:-}"
  local out
  if [[ -n "$plugin_dir" && -d "$plugin_dir" ]]; then
    out=$("$enc" --plugin-directory "$plugin_dir" --list-encoders 2>/dev/null || true)
  else
    out=$("$enc" --list-encoders 2>/dev/null || true)
  fi
  grep -q 'x265' <<<"$out"
}

bootstrap_heif_tools() {
  if [[ "${NO_BOOTSTRAP_HEIF:-}" == "1" ]]; then
    return 1
  fi
  command -v apt-get >/dev/null 2>&1 || return 1
  command -v dpkg-deb >/dev/null 2>&1 || return 1

  mkdir -p "$local_root/.deb-cache" "$local_root"
  (
    cd "$local_root/.deb-cache"
    apt-get download libheif-examples libheif-plugin-x265 >/dev/null
    for deb in ./*.deb; do
      dpkg-deb -x "$deb" "$local_root"
    done
  )
}

# Resolve heif-enc and x265 plugin. Prefer explicit env, then system install,
# then the user-local bootstrap copy.
heif_enc="${HEIF_ENC:-}"
heif_plugin_dir="${HEIF_PLUGIN_DIR:-}"
if [[ -n "$heif_enc" ]]; then
  [[ -x "$heif_enc" ]] || { echo "error: HEIF_ENC is not executable: $heif_enc" >&2; exit 1; }
elif command -v heif-enc >/dev/null 2>&1; then
  heif_enc="$(command -v heif-enc)"
elif [[ -x "$local_bin/heif-enc" ]]; then
  heif_enc="$local_bin/heif-enc"
else
  echo "heif-enc not found; bootstrapping libheif tools under $local_root ..." >&2
  bootstrap_heif_tools || {
    echo "error: heif-enc with x265 is required. Install libheif-examples and libheif-plugin-x265, or set HEIF_ENC/HEIF_PLUGIN_DIR." >&2
    exit 1
  }
  heif_enc="$local_bin/heif-enc"
fi

# If x265 is not visible system-wide, try the locally bootstrapped plugin dir.
if [[ -z "$heif_plugin_dir" && -d "$local_plugin_dir" ]]; then
  if has_x265_encoder "$heif_enc" "$local_plugin_dir"; then
    heif_plugin_dir="$local_plugin_dir"
  fi
fi
if ! has_x265_encoder "$heif_enc" "$heif_plugin_dir"; then
  echo "x265 encoder plugin not visible to heif-enc; bootstrapping local plugin ..." >&2
  bootstrap_heif_tools || true
  if [[ -d "$local_plugin_dir" ]] && has_x265_encoder "$heif_enc" "$local_plugin_dir"; then
    heif_plugin_dir="$local_plugin_dir"
  else
    echo "error: libheif x265 encoder plugin is required for HEIC/iOS compatibility" >&2
    exit 1
  fi
fi
if [[ -n "$heif_plugin_dir" ]]; then
  heif_enc_args+=(--plugin-directory "$heif_plugin_dir")
fi

heif_info="${HEIF_INFO:-}"
if [[ -n "$heif_info" ]]; then
  :
elif command -v heif-info >/dev/null 2>&1; then
  heif_info="$(command -v heif-info)"
elif [[ -x "$local_bin/heif-info" ]]; then
  heif_info="$local_bin/heif-info"
fi

mkdir -p "$(dirname "$output")"
tmpdir="$(mktemp -d)"
cleanup() { rm -rf "$tmpdir"; }
trap cleanup EXIT

job_json="$tmpdir/imageflow-normalize-rgb-png24.json"
intermediate_png="$tmpdir/intermediate-rgb.png"
tmp_output="$tmpdir/out.heic"

cat >"$job_json" <<JSON
{
  "io": [
    {"io_id": 0, "direction": "in", "io": "placeholder"},
    {"io_id": 1, "direction": "out", "io": "placeholder"}
  ],
  "framewise": {
    "steps": [
      {"decode": {"io_id": 0, "commands": []}},
      {"encode": {"io_id": 1, "preset": {"libpng": {"depth": "png_24", "matte": {"srgb": {"hex": "$matte_hex"}}, "zlib_compression": 6}}}}
    ]
  }
}
JSON

imageflow_tool v1/build --json "$job_json" --in "$input" --out "$intermediate_png" --quiet

"$heif_enc" "${heif_enc_args[@]}" \
  -e x265 \
  -q "$quality" \
  --no-alpha \
  -C sharp-yuv \
  --colour_primaries=1 \
  --transfer_characteristic=13 \
  --matrix_coefficients=6 \
  --full_range_flag=1 \
  "${description_args[@]}" \
  -o "$tmp_output" \
  "$intermediate_png" >/dev/null

mv "$tmp_output" "$output"

printf 'wrote %s\n' "$output"
if command -v file >/dev/null 2>&1; then
  file "$output"
fi
if [[ -n "$heif_info" && -x "$heif_info" ]]; then
  "$heif_info" "$output" | sed -n '1,18p'
fi
