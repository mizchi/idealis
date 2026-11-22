import json, sys, hashlib, os
log = sys.argv[1]
for line in open(log):
    line = line.strip()
    if not line.startswith('{'): continue
    m = json.loads(line)
    if m.get('reason') == 'compiler-artifact' and m.get('executable'):
        t = m['target']
        p = m['executable']
        h = hashlib.sha256(open(p,'rb').read()).hexdigest()
        print(t['name'], ','.join(t['kind']), p, h, m.get('fresh'))
