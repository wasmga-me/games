#!/usr/bin/env python3
"""Rewrites a .wasm's function names from Rust's v0 mangling to plain paths
(_RNvCs..._5crash10load_level becomes crash::load_level), so a crash trace
in the browser reads. Best effort: generic arguments and impl details are
dropped, and names it can't read stay as they are.

usage: wasm-names.py in.wasm out.wasm
"""
import re
import sys


def leb(data, i):
    n = shift = 0
    while True:
        b = data[i]
        i += 1
        n |= (b & 0x7F) << shift
        shift += 7
        if b < 0x80:
            return n, i


def enc(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        out.append(b | (0x80 if n else 0))
        if not n:
            return bytes(out)


def demangle(name):
    if not name.startswith("_R"):
        return name
    # crate roots and impls carry a disambiguator (s<base62>_), not a name
    body = re.sub(r"([CMXY])s[0-9A-Za-z]*_", r"\1", name[2:])
    parts, i = [], 0
    while i < len(body):
        m = re.match(r"(\d+)_?", body[i:])
        if m and int(m.group(1)) > 0:
            n = int(m.group(1))
            start = i + len(m.group(0))
            ident = body[start:start + n]
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", ident) and ident not in parts[-1:]:
                parts.append(ident)
            i = start + n
        else:
            i += 1
    return "::".join(parts) or name


def main(src, dst):
    data = bytearray(open(src, "rb").read())
    out, i = data[:8], 8
    while i < len(data):
        sid = data[i]
        size, j = leb(data, i + 1)
        end = j + size
        section = data[j:end]
        nlen, k = leb(section, 0)
        if sid == 0 and section[k:k + nlen] == b"name":
            body, p = bytearray(section[:k + nlen]), k + nlen
            while p < len(section):
                sub = section[p]
                ssize, q = leb(section, p + 1)
                content = section[q:q + ssize]
                if sub == 1:  # function names
                    count, r = leb(content, 0)
                    names = enc(count)
                    for _ in range(count):
                        idx, r = leb(content, r)
                        ln, r = leb(content, r)
                        name = demangle(content[r:r + ln].decode()).encode()
                        r += ln
                        names += enc(idx) + enc(len(name)) + name
                    content = names
                body += bytes([sub]) + enc(len(content)) + content
                p = q + ssize
            section = body
        out += bytes([sid]) + enc(len(section)) + section
        i = end
    open(dst, "wb").write(out)


if __name__ == "__main__":
    main(*sys.argv[1:3])
