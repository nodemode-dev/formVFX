import functools
import hashlib
import os
import re
import sys

RANGES = [
    (0x1F000, 0x1FAFF),
    (0x2600, 0x27BF),
    (0x2B05, 0x2B07),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B50),
    (0x2B55, 0x2B55),
    (0x2194, 0x2199),
    (0x21A9, 0x21AA),
    (0x231A, 0x231B),
    (0x2328, 0x2328),
    (0x23CF, 0x23CF),
    (0x23E9, 0x23F3),
    (0x23F8, 0x23FA),
    (0x25AA, 0x25AB),
    (0x25B6, 0x25B6),
    (0x25C0, 0x25C0),
    (0x25FB, 0x25FE),
    (0x2934, 0x2935),
    (0x3030, 0x3030),
    (0x303D, 0x303D),
    (0xFE0F, 0xFE0F),
    (0x200D, 0x200D),
]
ALLOWED = {0x21B5}
SKIP_DIRS = {"node_modules", "dist", "target", "venv", ".venv", ".git", "__pycache__", "build", "gen", "binaries"}
EXTS = (".md", ".ts", ".css", ".html", ".py", ".rs", ".toml", ".json", ".sh", ".bat", ".txt", ".yml", ".yaml", ".svg", ".js")
EM_DASH = "—"
NAMES = {
    "692f7c20ad3972ef": True, "352836dac99f04b7": False, "426b32f84949d29b": False, "9b1577b400b538b5": False,
    "63601b303a9ea9af": False, "0b6b5977ea29ef2b": False, "ec5e3719c28f4c47": True, "2f0bff2781f9cbd2": False,
    "3ddbd4d342c20431": True, "fa8a05cad2717b3e": False, "565188a5076cc800": True, "2fe5f7f1c10c2a01": True,
    "d717cc1729e21ca2": False, "cfec72dad0f73d10": False, "7a047a4c03ddd309": True, "278beafd903bd018": False,
    "b9f3892ae58d8272": False, "ad0a50b303ae59c2": True, "51850345b8ed6a85": False, "62cc20d8bc1e25c2": False,
    "5be22f8ce08e7958": True, "a3123b8a8bbbb0ac": True, "9fd588a8dd9d9e92": False,
}
DECLINED = {"3a91c6126b8ce74d", "67b8a41e4771da59", "18c3171c6b776c8f", "aaeeeaeb0dc11058", "541a6394f5c0e0d9",
            "502a0f420fe80695", "25ca52a5aeabb3b2"}
INTEROP = {("backend/app/routers/extensions.py", "352836dac99f04b7")}
TOKEN = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[0-9]+")


@functools.lru_cache(maxsize=1 << 18)
def digest(s: str) -> str:
    return hashlib.blake2b(s.encode(), digest_size=8).hexdigest()


def phrases(line: str) -> set:
    toks = TOKEN.findall(line)
    out = set()
    for i in range(len(toks)):
        for k in range(1, 5):
            if i + k <= len(toks):
                out.add("".join(toks[i:i + k]))
    return out


def third_party(rel: str, line: str, md: bool) -> list:
    out = []
    for p in phrases(line):
        exact, low = digest(p), digest(p.lower())
        for h in {x for x, cs in ((exact, True), (low, False)) if NAMES.get(x) is cs}:
            if (rel, h) not in INTEROP:
                out.append(("third-party product name", h))
        if not md and low in DECLINED:
            out.append(("declined patented feature", low))
    return out


def pictographic(cp: int) -> bool:
    return cp not in ALLOWED and any(a <= cp <= b for a, b in RANGES)


def scan(root: str) -> int:
    hits = 0
    for d, dirs, files in os.walk(root):
        dirs[:] = [x for x in dirs if x not in SKIP_DIRS]
        for f in files:
            if not f.endswith(EXTS) and "." in f:
                continue
            p = os.path.join(d, f)
            try:
                text = open(p, encoding="utf-8").read()
            except (UnicodeDecodeError, OSError):
                continue
            rel = os.path.relpath(p, root).replace(os.sep, "/")
            for n, line in enumerate(text.splitlines(), 1):
                for what, _ in third_party(rel, line, f.endswith(".md")):
                    print(f"{p}:{n}: {what}")
                    hits += 1
                for ch in line:
                    if pictographic(ord(ch)):
                        print(f"{p}:{n}: pictographic U+{ord(ch):04X}")
                        hits += 1
                if f.endswith(".md") and EM_DASH in line:
                    print(f"{p}:{n}: em dash in markdown")
                    hits += 1
    return hits


if __name__ == "__main__":
    roots = sys.argv[1:] or [os.path.dirname(os.path.dirname(os.path.abspath(__file__)))]
    total = sum(scan(r) for r in roots)
    print(f"lint_text: {total} finding(s)")
    sys.exit(1 if total else 0)
