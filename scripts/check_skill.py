"""Fails unless a SKILL.md starts with frontmatter that has a matching name and a description."""
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
if not text.startswith("---\n"):
    sys.exit(f"{path}: no frontmatter")
head, sep, _ = text[4:].partition("\n---\n")
if not sep:
    sys.exit(f"{path}: frontmatter is not closed")
fields = dict(line.split(":", 1) for line in head.splitlines() if ":" in line)
fields = {k.strip(): v.strip() for k, v in fields.items()}
if fields.get("name") != path.parent.name:
    sys.exit(f"{path}: name must be {path.parent.name!r}, got {fields.get('name')!r}")
if len(fields.get("description", "")) < 20:
    sys.exit(f"{path}: description is missing or too short")
for ref in ("references/error-codes.md", "references/anchor-ranges.md", "references/helius-endpoints.md"):
    if not (path.parent / ref).exists():
        sys.exit(f"{path}: missing {ref}")
print(f"{path}: ok")
