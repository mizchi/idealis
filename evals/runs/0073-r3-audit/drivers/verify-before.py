import json, hashlib, sys
root = sys.argv[1]
m = json.load(open(f'{root}/manifest.json'))
print('plan', m['plan'], 'revision', m['revision'])
bad = 0
for f in m['files']:
    h = hashlib.sha256(open(f"{root}/{f['path']}", 'rb').read()).hexdigest()
    print(f"{f['path']} {h} {'OK' if h == f['sha256'] else 'MISMATCH expected ' + f['sha256']}")
    bad += h != f['sha256']
print('manifest sha256', hashlib.sha256(open(f'{root}/manifest.json', 'rb').read()).hexdigest())
sys.exit(1 if bad else 0)
