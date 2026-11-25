# usage: analyze-tlc.py <tlc dir> <0074 verdicts.json> <0075 verdicts.json> <paths dir> <cases-0119.json> <out dir>
import json, re, sys, collections
tlcdir, old74, old75, pathsdir, cases119, out = sys.argv[1:7]
tlc = json.load(open(f'{tlcdir}/results.json'))
A = json.load(open(f'{pathsdir}/after.json'))['cases']
B = json.load(open(f'{pathsdir}/before.json'))['cases']
def verdict(v):
    log = open(f"{v['dir']}/tlc.log").read()
    if v['completed']: return ['complete', v['states'], v['generated']]
    if re.search(r'Invariant \w+ is violated', log): return ['violation', max(v['trace'], 1)]
    if re.search(r'second argument of \\div is 0|division by zero', log): return ['division-by-zero']
    if re.search(r'non-boolean expression|when [PQ] was', log): return ['non-boolean']
    if re.search(r'Parsing or semantic analysis failed', log): return ['semantic-error']
    return ['other-error', v['errors'][:2]]
def ours(o):
    s = o['stage']
    if s == 'result':
        if o['status'] == 'complete': return ['complete', o['states'], o['generated']]
        if o['status'] == 'bounded': return ['bounded', o['reason']]
        return ['violation', o['trace']]
    m = o['message']
    if m == 'Integer division by zero': return ['division-by-zero']
    if m.startswith('Expected a boolean'): return ['non-boolean']
    if m.startswith('recursion limit exceeded'): return ['limit', 'request decoder 128']
    for key, label in [('Safety expression nesting exceeds 48', 'checker 48'), ('Safety evaluation nesting exceeds 128', 'evaluation 128'),
                       ('Safety evaluation nesting exceeds 256', 'evaluation 256'),
                       ('compiled expression nesting exceeds 128', 'frontend output 128'), ('expression or operator nesting exceeds 40', 'frontend 40'),
                       ('TLA parsing: Expression nesting exceeds 64', 'parser 64'), ('expression Apply', 'TS lacks operator'),
                       ('modules or EXTENDS', 'TS lacks EXTENDS'), ('Safety branch exceeds 256 steps', 'branch 256'),
                       ('recursive operator definitions', 'TS lacks RECURSIVE')]:
        if m.startswith(key): return ['limit', label]
    return [s + '-refusal', m]
verdicts = {n: verdict(v) for n, v in tlc.items()}
cited = {}
for path in (old74, old75):
    for n, v in json.load(open(path)).items():
        cited.setdefault(n, v)
json.dump(verdicts, open(f'{out}/verdicts.json', 'w'), indent=1)
lines = [f'TLC runs: {len(tlc)}; verdicts: {dict(collections.Counter(v[0] for v in verdicts.values()))}']
cnt = collections.Counter(); bad = []
for side, D in (('before', B), ('after', A)):
    for name, tv in verdicts.items():
        for ver in ('v1', 'v2'):
            for path in ('rust-native', 'ts-native'):
                o = ours(D[name][ver][path]['outcome'])
                cls = 'match' if o == tv else ('limit (' + o[1] + ')' if o[0] in ('limit', 'bounded') else 'MISMATCH')
                cnt[(side, ver, path, cls)] += 1
                if cls == 'MISMATCH': bad.append((name, side, ver, path, tv, o))
lines += [f'{c} {k}' for k, c in sorted(cnt.items())]
lines.append(f'MISMATCH rows: {len(bad)}'); lines += [str(b) for b in bad]
changed = collections.Counter(); changedbad = []
for n in A:
    if n not in cited or n in verdicts: continue
    tv = cited[n]
    if tv[0] == 'semantic-error': tv = ['semantic-error']
    for path in ('rust-native', 'ts-native'):
        b, a = B[n]['v2'][path]['outcome'], A[n]['v2'][path]['outcome']
        if a == b: continue
        o = ours(a)
        cls = 'match' if o == tv else ('limit (' + o[1] + ')' if o[0] == 'limit' else 'MISMATCH')
        changed[(path, cls)] += 1
        if cls == 'MISMATCH': changedbad.append((n, path, tv, o))
lines.append(f'changed v2 cells of earlier inputs against the cited TLC verdicts: {dict(changed)}'); lines += [str(b) for b in changedbad]
open(f'{out}/analysis.txt', 'w').write('\n'.join(lines) + '\n'); print('\n'.join(lines))
