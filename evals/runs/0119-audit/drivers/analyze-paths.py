# usage: analyze-paths.py <paths dir> <cases.json> <cases-0075.json> <cases-0119.json>; writes analysis.txt and differences.tsv in <paths dir>
import json, sys, collections
d, c1, c2, c3 = sys.argv[1:5]
side = {s: json.load(open(f'{d}/{s}.json'))['cases'] for s in ['before', 'oracle', 'after']}
cases = {c['name']: c for c in json.load(open(c1)) + json.load(open(c2)) + json.load(open(c3))}
paths = ['ts-wasm', 'ts-native', 'rust-wasm', 'rust-native']
front = {'ts-wasm': 'ts', 'ts-native': 'ts', 'rust-wasm': 'rust', 'rust-native': 'rust'}
LIMITS = ('Safety evaluation nesting exceeds 128',)
CURRENT = 'Safety evaluation nesting exceeds 512'
out = []; diffs = []
def o(cell): return cell['outcome']
def comparable(actual, expected):
    if actual.get('stage') == 'result' and expected.get('stage') == 'result' and expected.get('work') is None:
        a = dict(actual); a['work'] = None; return a
    return actual
for s in ['before', 'after', 'oracle']:
    n = sum(1 for c in side[s].values() for v in c.values() for a, b in [('ts-wasm', 'ts-native'), ('rust-wasm', 'rust-native')] if o(v[a]) != o(v[b]))
    exc = sum(1 for c in side[s].values() for v in c.values() for p in paths if o(v[p]).get('stage') == 'exception')
    out.append(f'[{s}] cells: {sum(len(c)*4 for c in side[s].values())}; wasm vs native outcome differs: {n}; exceptions: {exc}')
irb = sum(1 for name, c in side['after'].items() for v, cell in c.items() for p in paths if cell[p]['ir'] != side['before'][name][v][p]['ir'])
out.append(f'before vs after IR sha256 differs (all cells incl. refusals as null): {irb}')
tsrust = collections.Counter()
for s in ['before', 'after']:
    for name, c in side[s].items():
        for v, cell in c.items():
            a, b = cell['ts-native']['irNormalized'], cell['rust-native']['irNormalized']
            if a and b: tsrust[(s, 'equal' if a == b else 'differ')] += 1
out.append(f'TS vs Rust normalized IR where both compile: {dict(tsrust)}')
v1 = sum(1 for name, c in side['after'].items() for p in paths if o(c['v1'][p]) != o(side['before'][name]['v1'][p]))
out.append(f'v1 before vs after outcome differs: {v1}')
cls = collections.Counter(); other = []; recursive = []
for name, c in side['after'].items():
    for p in paths:
        b, a, r = o(side['before'][name]['v2'][p]), o(c['v2'][p]), o(side['oracle'][name]['v2'][p])
        if a == b: continue
        limited = b.get('stage') == 'check' and b.get('message') in LIMITS
        rec = b.get('stage') == 'result' and b.get('status') == 'bounded'
        later = a.get('stage') == 'result' or (a.get('stage') in ('check', 'compile') and a.get('message') not in LIMITS)
        kind = ('allowed' if (limited or rec) and later else 'OTHER')
        tag = f"{kind}: {b.get('message', b.get('status'))[:40]} -> {a.get('stage')} {a.get('status', a.get('message', ''))[:40]}"
        cls[tag] += 1
        oracle_same = a == r
        diffs.append('\t'.join([name, p, json.dumps(b), json.dumps(a), json.dumps(r), 'oracle-equal' if oracle_same else 'oracle-differs']))
        if kind == 'OTHER': other.append((name, p, b, a))
        if not oracle_same: recursive.append((name, p, a.get('status', a.get('message')), r.get('status', r.get('message')), a.get('work'), r.get('work')))
out.append(f'v2 before vs after differing cells: {sum(cls.values())}')
out += [f'  {v} {k}' for k, v in sorted(cls.items())]
out.append(f'OTHER differences: {len(other)}'); out += [f'  {x}' for x in other]
out.append(f'changed cells where after differs from the oracle: {len(recursive)}'); out += [f'  {x}' for x in recursive]
mism = []
for name, c in side['after'].items():
    if name.startswith('quint:'): continue
    for v in ['v1', 'v2']:
        for p in paths:
            e = cases[name]['expect'][front[p]][v]
            if comparable(o(c[v][p]), e) != e: mism.append((name, v, p, o(c[v][p]), e))
out.append(f'after vs recorded expectations mismatches: {len(mism)}'); out += [f'  {x}' for x in mism[:20]]
for name in sorted(n for n in side['after'] if n.startswith('quint:')):
    out.append(f"{name}: " + ' | '.join(f"{s} {json.dumps(o(side[s][name]['v2']['rust-native']))}" for s in ['before', 'oracle', 'after']))
open(f'{d}/analysis.txt', 'w').write('\n'.join(out) + '\n')
open(f'{d}/differences.tsv', 'w').write('case\tpath\tbefore\tafter\toracle\toracle\n' + '\n'.join(diffs) + '\n')
print('\n'.join(out))
