#!/bin/sh
# All-history secret scan. Fail-closed: a scan that cannot run is never 'clean'.
# Prints only commit:path for findings; never the matched secret value.
set -u

# 1. git must be available.
if ! command -v git >/dev/null 2>&1; then
    printf 'scan-history: git not found; cannot scan history\n' >&2
    exit 2
fi

# 2. Full history is required; a shallow clone cannot be trusted as clean.
shallow="$(git rev-parse --is-shallow-repository 2>/dev/null)" || {
    printf 'scan-history: cannot determine repository depth\n' >&2
    exit 2
}
case "$shallow" in
    false) ;;
    true)
        printf 'scan-history: shallow repository; cannot scan full history\n' >&2
        exit 2
        ;;
    *)
        printf 'scan-history: unexpected repository depth state: %s\n' "$shallow" >&2
        exit 2
        ;;
esac

TMP="$(mktemp -d)" || {
    printf 'scan-history: cannot create temp directory\n' >&2
    exit 2
}
trap 'rm -rf "$TMP"' 0 HUP INT TERM

# 3. Map each blob SHA to a commit:path from the full commit graph.
if ! git log --all --format='COMMIT %H' --raw --no-abbrev --no-renames --root \
        > "$TMP/log" 2>/dev/null; then
    printf 'scan-history: cannot walk commits\n' >&2
    exit 2
fi
awk '
    /^COMMIT / { commit = $2; next }
    /^:/ {
        split($0, parts, "\t")
        split(parts[1], meta, " ")
        sha = meta[4]
        if (sha ~ /^[0-9a-f]+$/ && parts[2] != "")
            print sha "\t" commit ":" parts[2]
    }
' "$TMP/log" > "$TMP/map"

# 4. Enumerate every object reachable from all refs, then read all contents in
#    one cat-file --batch process. Both steps are mandatory for a valid scan.
if ! git rev-list --objects --all > "$TMP/objects_raw" 2>/dev/null; then
    printf 'scan-history: cannot enumerate objects\n' >&2
    exit 2
fi
awk '{ print $1 }' "$TMP/objects_raw" > "$TMP/objects"
if ! git cat-file --batch < "$TMP/objects" > "$TMP/batch" 2>/dev/null; then
    printf 'scan-history: cannot read object contents\n' >&2
    exit 2
fi

# 5. Realistic token shapes: documentation literals (bare ghp_, sk-[...], etc.)
#    do not match, but a real-length credential does.
PATTERNS='ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|gho_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{35}|sk-[A-Za-z0-9]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|(API_KEY|TOKEN|SECRET|PASSWORD)[[:space:]]*=[[:space:]]*[A-Za-z0-9/+_-]{20,}|Bearer [A-Za-z0-9._-]{30,}'

# cat-file --batch emits "<sha> <type> <size>\n" before each object's bytes.
# Track the current blob from those headers; a matching content line marks it.
grep -aE "^[0-9a-f]+ (blob|commit|tree) |$PATTERNS" "$TMP/batch" |
    awk '
        /^[0-9a-f]+ blob / { cur = $1; next }
        /^[0-9a-f]+ (commit|tree) / { cur = ""; next }
        cur != "" { print cur }
    ' | sort -u > "$TMP/matched"

# 6. Report commit:path only. Never echo the matched value.
found=0
while IFS= read -r sha; do
    [ -n "$sha" ] || continue
    label="$(awk -F '\t' -v s="$sha" '$1 == s { print $2; exit }' "$TMP/map")"
    if [ -n "$label" ]; then
        printf '%s\n' "$label"
    else
        printf '%s:<unknown>\n' "$sha"
    fi
    found=1
done < "$TMP/matched"

if [ "$found" -eq 1 ]; then
    exit 1
fi
exit 0
