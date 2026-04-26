#!/usr/bin/env bash
# Fetches third-party JS libraries used for office/pdf preview.
# Idempotent: skips files that already exist.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="$ROOT/ui/vendor"

PDFJS_VERSION="4.10.38"
MAMMOTH_VERSION="1.8.0"

PDFJS_BASE="https://cdnjs.cloudflare.com/ajax/libs/pdf.js/${PDFJS_VERSION}"
MAMMOTH_BASE="https://cdnjs.cloudflare.com/ajax/libs/mammoth/${MAMMOTH_VERSION}"

mkdir -p "$VENDOR/pdfjs" "$VENDOR/mammoth"

fetch() {
  local url="$1" out="$2"
  if [[ -s "$out" ]]; then
    echo "✓ already exists: ${out#$ROOT/}"
    return 0
  fi
  echo "↓ $url"
  curl -fL --retry 3 --retry-delay 2 -o "$out.tmp" "$url"
  mv "$out.tmp" "$out"
}

fetch "$PDFJS_BASE/pdf.min.mjs"        "$VENDOR/pdfjs/pdf.min.mjs"
fetch "$PDFJS_BASE/pdf.worker.min.mjs" "$VENDOR/pdfjs/pdf.worker.min.mjs"
fetch "$MAMMOTH_BASE/mammoth.browser.min.js" "$VENDOR/mammoth/mammoth.browser.min.js"

cat > "$VENDOR/LICENSES.md" <<'EOF'
# Third-party libraries

## PDF.js — Apache License 2.0
Source: https://github.com/mozilla/pdf.js
Files: `pdfjs/pdf.min.mjs`, `pdfjs/pdf.worker.min.mjs`

## mammoth.js — BSD 2-Clause License
Source: https://github.com/mwilliamson/mammoth.js
Files: `mammoth/mammoth.browser.min.js`
EOF

echo "Done."
