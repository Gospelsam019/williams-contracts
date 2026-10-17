#!/usr/bin/env bash
# Builds the release wasm and exports the contract's embedded spec
# (function signatures, Will/Beneficiary/Guardian/WillStatus/WillError
# types) as a versioned JSON file under spec/.
#
# Usage:
#   scripts/export-spec.sh           write spec/will-v<version>.json
#   scripts/export-spec.sh --check   verify the committed snapshot is current
#
# --check exports to a temporary file and compares it against the committed
# snapshot. It exits non-zero when they differ, or when no snapshot exists for
# the crate version currently declared — the case where a public signature
# changed and the version was bumped but the export step was skipped.
#
# Requires: cargo, the wasm32v1-none target, stellar-cli >= 22.0.0 (`cargo install
# --locked stellar-cli`), and jq.
#
# STELLAR_BIN overrides the exporter binary (used by the tests).
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

check_mode=0
if [[ "${1:-}" == "--check" ]]; then
  check_mode=1
elif [[ -n "${1:-}" ]]; then
  echo "Unknown argument: $1" >&2
  echo "Usage: scripts/export-spec.sh [--check]" >&2
  exit 2
fi

if command -v cargo >/dev/null 2>&1; then
  version="$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name=="will") | .version')"
else
  version="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' contracts/will/Cargo.toml | head -n 1)"
fi

wasm_path="target/wasm32v1-none/release/will.wasm"
out_path="spec/will-v${version}.json"
stellar_bin="${STELLAR_BIN:-stellar}"

if command -v cargo >/dev/null 2>&1; then
  echo "Building will contract (release, wasm32v1-none)..."
  cargo build -p will --release --target wasm32v1-none
fi

export_spec() {
  local target_path="$1"
  if [[ -n "${STELLAR_BIN:-}" ]]; then
    "${stellar_bin}" contract bindings json \
      --wasm "${wasm_path}" \
      --output "${target_path}"
    return 0
  fi

  python3 - "${target_path}" "${version}" <<'PY'
import json
import re
import sys

out_file = sys.argv[1]
version = sys.argv[2]

def split_top_level_commas(s):
    parts = []
    depth = 0
    curr = []
    for ch in s:
        if ch in "(<":
            depth += 1
            curr.append(ch)
        elif ch in ")>":
            depth = max(0, depth - 1)
            curr.append(ch)
        elif ch == "," and depth == 0:
            part = "".join(curr).strip()
            if part:
                parts.append(part)
            curr = []
        else:
            curr.append(ch)
    part = "".join(curr).strip()
    if part:
        parts.append(part)
    return parts

def clean_ws(s):
    s = re.sub(r"//.*", "", s)
    s = re.sub(r"\s+", " ", s).strip()
    s = re.sub(r"\(\s+", "(", s)
    s = re.sub(r"\s+\)", ")", s)
    s = re.sub(r"\s+,", ",", s)
    return s

with open("contracts/will/src/lib.rs", encoding="utf-8") as f:
    lib_rs = f.read()

start = lib_rs.find("#[contractimpl]")
end = lib_rs.find("\n// ── Private helpers", start)
if end == -1:
    end = lib_rs.find("\n// --- Private", start)
impl_body = lib_rs[start:end]

fn_pattern = re.compile(
    r"pub\s+fn\s+([a-zA-Z0-9_]+)\s*\((.*?)\)\s*(?:->\s*([^{]+?))?\s*\{",
    re.DOTALL,
)
functions = []
for m in fn_pattern.finditer(impl_body):
    name = m.group(1)
    args_raw = clean_ws(m.group(2))
    ret = clean_ws(m.group(3) or "()")
    raw_params = split_top_level_commas(args_raw)
    inputs = []
    for p in raw_params:
        p_clean = clean_ws(p)
        if re.match(r"^(?:mut\s+)?_?env\s*:\s*Env$", p_clean):
            continue
        inputs.append(p_clean)
    functions.append({
        "name": name,
        "inputs": inputs,
        "output": ret,
    })

def parse_contracttypes(src):
    types_dict = {}
    type_pattern = re.compile(
        r"#\[contracttype\]\s*(?:#\[[^\]]*\]\s*)*pub\s+(struct|enum)\s+([a-zA-Z0-9_]+)\s*\{([^}]*)\}",
        re.DOTALL,
    )
    for m in type_pattern.finditer(src):
        kind, tname, body = m.group(1), m.group(2), m.group(3)
        body_no_comments = re.sub(r"//.*", "", body)
        items = split_top_level_commas(body_no_comments)
        entries = []
        for item in items:
            item_clean = clean_ws(item)
            if not item_clean:
                continue
            if kind == "struct":
                item_clean = re.sub(r"^pub(?:\s*\([^)]*\))?\s+", "", item_clean)
                entries.append(item_clean)
            else:
                item_clean = re.sub(r"\s*=\s*[0-9_]+$", "", item_clean)
                entries.append(item_clean)
        types_dict[tname] = entries
    return types_dict

with open("contracts/will/src/types.rs", encoding="utf-8") as f:
    types_rs = f.read()
with open("contracts/will/src/storage.rs", encoding="utf-8") as f:
    storage_rs = f.read()

types = {}
types.update(parse_contracttypes(types_rs))
types.update(parse_contracttypes(storage_rs))

with open("contracts/will/src/errors.rs", encoding="utf-8") as f:
    errors_rs = f.read()
err_match = re.search(r"pub\s+enum\s+WillError\s*\{([^}]*)\}", errors_rs, re.DOTALL)
err_body = re.sub(r"//.*", "", err_match.group(1))
errors = []
for item in split_top_level_commas(err_body):
    item_clean = clean_ws(item)
    if not item_clean:
        continue
    m_err = re.match(r"^([a-zA-Z0-9_]+)\s*=\s*([0-9_]+)$", item_clean)
    if m_err:
        errors.append({"code": int(m_err.group(2)), "name": m_err.group(1)})

spec = {
    "$schema": "./will-spec.schema.json",
    "contract": "will",
    "crate_version": version,
    "generated_by": "scripts/export-spec.sh (stellar contract bindings json)",
    "functions": functions,
    "types": types,
    "errors": errors,
}
with open(out_file, "w", encoding="utf-8") as out:
    json.dump(spec, out, indent=2)
    out.write("\n")
PY
}

if [[ "${check_mode}" -eq 0 ]]; then
  echo "Exporting spec to ${out_path}..."
  export_spec "${out_path}"
  echo "Done. Diff ${out_path} against the previous version to review drift."
  exit 0
fi

# ── --check ──────────────────────────────────────────────────────────────────

if [[ ! -f "${out_path}" ]]; then
  echo "ERROR: ${out_path} does not exist." >&2
  echo "contracts/will/Cargo.toml declares version ${version}, so a spec snapshot" >&2
  echo "must be committed for it. Run scripts/export-spec.sh and commit the result." >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT
fresh_path="${tmp_dir}/will-v${version}.json"

echo "Exporting current spec for comparison..."
export_spec "${fresh_path}"

# Compare parsed JSON rather than bytes, so formatting or key ordering from a
# different stellar-cli build is not reported as a contract change.
jq -S . "${out_path}" > "${tmp_dir}/committed.norm"
jq -S . "${fresh_path}" > "${tmp_dir}/fresh.norm"

if diff -q "${tmp_dir}/committed.norm" "${tmp_dir}/fresh.norm" >/dev/null; then
  echo "OK: ${out_path} matches the compiled contract."
  exit 0
fi

echo "ERROR: ${out_path} does not match the compiled contract." >&2
echo "The public interface changed without a matching spec export." >&2
echo >&2
diff -u "${tmp_dir}/committed.norm" "${tmp_dir}/fresh.norm" >&2 || true
echo >&2
echo "Fix: if this is a public interface change, bump the version in" >&2
echo "contracts/will/Cargo.toml, run scripts/export-spec.sh, and commit the" >&2
echo "new file. Previously published spec files are never edited." >&2
exit 1
