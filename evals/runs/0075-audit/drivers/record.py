# usage: record.py <source dir> <run dir (must not exist)> <kept glob>...; copies kept files with host paths replaced by placeholders and records the rest in reduced.json
import sys, os, glob, hashlib, json, fnmatch
source, target, kept = sys.argv[1], sys.argv[2], sys.argv[3:]
E = "<scratch>"
W = "<after>"
REPLACE = [(E + "/before", "<before>"), (E + "/oracle", "<oracle>"), (E + "/red", "<red>"), (E, "<scratch>"), (W, "<after>"),
           ("<tmp>", "<tmp>"), ("<tmp>", "<tmp>"),
           ("<mise>", "<mise>"), ("<home>", "<home>"), ("<user>", "<user>")]
def scrub(text):
    for a, b in REPLACE: text = text.replace(a, b)
    return text
os.makedirs(target)
removed = []
for root, _, files in os.walk(source):
    for name in sorted(files):
        path = os.path.join(root, name); rel = os.path.relpath(path, source)
        data = open(path, 'rb').read()
        if any(fnmatch.fnmatch(rel, k) for k in kept):
            out = os.path.join(target, rel); os.makedirs(os.path.dirname(out), exist_ok=True)
            try: open(out, 'w').write(scrub(data.decode()))
            except UnicodeDecodeError: open(out, 'wb').write(data)
        else:
            removed.append((rel, len(data), hashlib.sha256(data).hexdigest()))
if removed:
    removed.sort()
    listing = ''.join(f"{h}  {r}\n" for r, _, h in removed)
    json.dump({"note": "Raw outputs removed from this run; the listing digest is sha256 over sha256sum lines of the removed files sorted by path.",
               "removed": {"files": len(removed), "bytes": sum(b for _, b, _ in removed), "listingSha256": hashlib.sha256(listing.encode()).hexdigest(),
                           "large": [{"path": r, "bytes": b, "sha256": h} for r, b, h in removed if b >= 262144]}},
              open(os.path.join(target, 'reduced.json'), 'w'), indent=2)
print(target, 'kept', sum(1 for _ in glob.glob(target + '/**', recursive=True)), 'removed', len(removed))
