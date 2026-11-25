# Read the rustflags a committed Cargo config brings, for .githooks/pre-commit.
#
# Prints four lines:
#   1. the rustflags from [build], space separated (empty when there are none)
#   2. how many non-comment lines mention "rustflags" at all
#   3. 1 when every non-comment rustflags is in [build], 0 when one is elsewhere
#   4. 1 when the [build] array was read to its closing bracket, 0 when it was not
#
# Lines 3 and 4 are what let the hook fail closed: rustflags under [target.*]
# (or any other table), and arrays it cannot read, are not reproduced here, so
# the hook must refuse rather than lint content that differs from what the
# commit's own build would use.
#
# POSIX awk only. Handles a single-line array, a multi-line array, an empty
# array, and both TOML string forms: basic strings ("...", with escapes) and
# literal strings ('...', taken verbatim). Comment lines are ignored, because
# Cargo ignores them: treating a commented-out rustflags as real would apply a
# setting the commit does not have.

# Unquote one TOML array, honouring "..." (with escapes) and '...' (literal).
function unquote(text,   out, i, n, c, next_c, value) {
    out = ""
    n = length(text)
    i = 1
    while (i <= n) {
        c = substr(text, i, 1)
        if (c == "\"") {
            value = ""
            i += 1
            while (i <= n) {
                next_c = substr(text, i, 1)
                if (next_c == "\\") {
                    value = value substr(text, i + 1, 1)
                    i += 2
                    continue
                }
                if (next_c == "\"") {
                    i += 1
                    break
                }
                value = value next_c
                i += 1
            }
            out = (out == "" ? value : out " " value)
            continue
        }
        if (c == "'") {
            value = ""
            i += 1
            while (i <= n) {
                next_c = substr(text, i, 1)
                if (next_c == "'") {
                    i += 1
                    break
                }
                value = value next_c
                i += 1
            }
            out = (out == "" ? value : out " " value)
            continue
        }
        i += 1
    }
    return out
}

BEGIN {
    section = ""
    collecting = 0
    buf = ""
    mentions = 0
    elsewhere = 0
    parsed_ok = 1
}

{
    line = $0
    sub(/^[[:space:]]*/, "", line)

    # A comment is not a setting. In a multi-line array a comment is also not
    # the closing bracket, so it must not end the collection either.
    if (line ~ /^#/) {
        next
    }

    if (collecting) {
        buf = buf $0 "\n"
        if (line ~ /\]/) {
            collecting = 0
            parsed_ok = 1
        }
        next
    }

    if (line ~ /^\[/) {
        section = line
        gsub(/[[:space:]]/, "", section)
        next
    }

    # Anchor the key so only a real `rustflags = ...` (or a quoted key) counts.
    if (line !~ /^"?rustflags"?[[:space:]]*=/) {
        next
    }

    mentions += 1
    if (section != "[build]") {
        elsewhere += 1
        next
    }

    value = line
    sub(/^[^=]*=/, "", value)
    # An empty array that closes on the same line means "no rustflags", which
    # is safe to treat as such; anything else must be read to its bracket.
    if (!(value ~ /\[[[:space:]]*\]/)) {
        parsed_ok = (value ~ /\]/) ? 1 : 0
    }
    buf = value "\n"
    collecting = (value ~ /\]/) ? 0 : 1
}

END {
    printf "%s\n%d\n%d\n%d\n", unquote(buf), mentions, (elsewhere == 0), parsed_ok
}
