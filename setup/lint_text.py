import os
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
            for n, line in enumerate(text.splitlines(), 1):
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
