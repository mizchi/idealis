# usage: split-chromium.py <chromium.json> <output prefix>; writes <prefix>-main.json and <prefix>-worker.json
import json, sys
data = json.load(open(sys.argv[1]))
for host in ("main", "worker"):
    json.dump(data[host], open(f"{sys.argv[2]}-{host}.json", "x"), indent=1)
