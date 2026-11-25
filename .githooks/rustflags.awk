# Read the rustflags a committed Cargo config brings, for .githooks/pre-commit.
#
# Prints four lines:
#   1. the rustflags from [build], space separated (empty when there are none)
#   2. how many lines that are not comments contain a rustflags key
#   3. 1 when every such key is in [build], 0 when one is elsewhere
#   4. 1 when the [build] array was read to its closing bracket, 0 when it was not
#
# Lines 3 and 4 let the hook fail closed: rustflags under [target.*] (or any
# other table), and arrays it cannot read, are not reproduced here, so the hook
# must refuse rather than lint content that differs from the commit's build.
#
# POSIX awk only. Handles a single-line array, a multi-line array, an empty
# array, and both TOML string forms ("..." with escapes, '...' literal).
# Comments are removed quote-aware before anything is read, because a "]" or a
# quote inside a comment is prose: applying it would use a flag the commit does
# not have, and letting it end an array would drop flags the commit does have.

# Return $0 with any trailing comment removed. A "#" starts a comment only when
# it is outside a string, so a value like "a#b" survives.
function strip_comment(text,   out, i, n, c, quote) {
    out = ""
    quote = ""
    n = length(text)
    for (i = 1; i <= n; i++) {
        c = substr(text, i, 1)
        if (quote == "") {
            if (c == "#") {
                break
            }
            if (c == "\"" || c == "'") {
                quote = c
            }
            out = out c
            continue
        }
        if (quote == "\"" && c == "\\") {
            out = out c substr(text, i + 1, 1)
            i += 1
            continue
        }
        if (c == quote) {
            quote = ""
        }
        out = out c
    }
    return out
}

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

# 1 when the closing "]" of an array appears outside a string in this text.
function closes_array(text,   i, n, c, quote) {
    quote = ""
    n = length(text)
    for (i = 1; i <= n; i++) {
        c = substr(text, i, 1)
        if (quote == "") {
            if (c == "\"" || c == "'") {
                quote = c
                continue
            }
            if (c == "]") {
                return 1
            }
            continue
        }
        if (quote == "\"" && c == "\\") {
            i += 1
            continue
        }
        if (c == quote) {
            quote = ""
        }
    }
    return 0
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
    line = strip_comment($0)
    sub(/^[[:space:]]*/, "", line)
    if (line == "") {
        next
    }

    if (collecting) {
        buf = buf line "\n"
        if (closes_array(line)) {
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
    # An empty array that closes on the same line means "no rustflags", which is
    # safe to treat as such; anything else must be read to its closing bracket.
    if (value ~ /\[[[:space:]]*\]/) {
        parsed_ok = 1
        collecting = 0
    } else {
        collecting = closes_array(value) ? 0 : 1
        parsed_ok = collecting ? 0 : 1
    }
    buf = value "\n"
}

END {
    printf "%s\n%d\n%d\n%d\n", unquote(buf), mentions, (elsewhere == 0), parsed_ok
}
