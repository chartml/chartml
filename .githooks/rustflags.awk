# Read the rustflags a committed Cargo config brings, for .githooks/pre-commit.
#
# Prints three lines:
#   1. the rustflags from [build], space separated (empty when there are none)
#   2. how many lines mention "rustflags" at all
#   3. 1 when every rustflags line is in [build], 0 when one is elsewhere
#
# Line 3 is what lets the hook fail closed: rustflags under [target.*] (or any
# other table) are not reproduced here, so the hook must refuse rather than
# lint content that differs from what the commit's own build would use.
#
# POSIX awk only. Supports a single-line array, a multi-line array, an empty
# array, quoted elements and escaped quotes.

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
    pending_value = ""
}

{
    line = $0
    sub(/^[[:space:]]*/, "", line)

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

    if (line ~ /rustflags/) {
        mentions += 1
        if (section != "[build]") {
            elsewhere += 1
            next
        }
        # Only the value side: a quoted key (`"rustflags" = [...]`) still reads
        # as rustflags, but the key text itself must not become a flag.
        value = line
        if (value ~ /=/) {
            sub(/^[^=]*=/, "", value)
        } else if (value ~ /\./) {
            sub(/^[^.]*\./, "", value)
        }
        # An empty array that closes on the same line means "no rustflags",
        # which is safe to treat as such; anything else must be readable.
        if (!(value ~ /\[[[:space:]]*\]/)) {
            parsed_ok = (value ~ /\]/) ? 1 : 0
            pending_value = value
        }
        buf = value "\n"
        collecting = (value ~ /\]/) ? 0 : 1
    }
}

END {
    printf "%s\n%d\n%d\n%d\n", unquote(buf), mentions, (elsewhere == 0), parsed_ok
}
