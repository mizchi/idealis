import json, sys
summary, ref = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else None)
d = json.load(open(f'{summary}/result.json'))
m = d['manifest']
print('binarySha256', [x['binarySha256'][:12] for x in m['metadata']], 'order', m['order'], 'sampleSize', m['sampleSize'], 'warmupMs', m['warmupMs'], 'measurementMs', m['measurementMs'])
refr = {}
if ref:
    refr = {r['name']: r['ratio'] for r in json.load(open(f'{ref}/result.json'))['records']}
print(f"{'kernel':<36}{'before ns':>12}{'after ns':>12}{'change %':>9}  rep0(B) rep1(A) rep2(A) rep3(B) ns  flagged>=10%  spreadB spreadA  r2 change %")
ch = []
for r in d['records']:
    e = [x['mean']['point_estimate'] for x in r['estimates']]
    sb = abs(e[0] - e[3]) / ((e[0] + e[3]) / 2) * 100
    sa = abs(e[1] - e[2]) / ((e[1] + e[2]) / 2) * 100
    c = (r['ratio'] - 1) * 100
    ch.append(c)
    rr = f"{(refr[r['name']] - 1) * 100:+.2f}" if r['name'] in refr else ''
    print(f"{r['name']:<36}{r['beforeNs']:>12.0f}{r['afterNs']:>12.0f}{c:>+9.2f}  {e[0]:.0f} {e[1]:.0f} {e[2]:.0f} {e[3]:.0f}  {'YES' if r['regressionOver10Percent'] else 'no':>5}  {sb:5.1f}% {sa:5.1f}%  {rr}")
print('count', len(ch), 'max change', max(ch), 'min change', min(ch), 'flagged', sum(1 for r in d['records'] if r['regressionOver10Percent']))
