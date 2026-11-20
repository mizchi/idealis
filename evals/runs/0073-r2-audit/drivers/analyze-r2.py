import json, statistics as st
E='<scratch>/eval-0073-r2/runs'
r=json.load(open(f'{E}/0073-r2-nondet-native-rss/result.json'))
def strip(x):
    y=dict(x); y.pop('work',None); return y
print('status', r['status'], 'samples', len(r['samples']), 'controls', len(r['controls']))
print(f"{'w':>2} {'before med s':>12} {'after med s':>11} {'ratio':>6} {'states':>6} {'generated':>9} {'RSS b MiB':>9} {'RSS a MiB':>9} {'exceptWorkEq':>12} {'work before':>12} {'work after':>11}  samples B/A")
for w in (1,2,4,8):
    S=[s for s in r['samples'] if s['workers']==w]
    nb=[s for s in S if s['side']=='before' and not s['warmup']]; na=[s for s in S if s['side']=='after' and not s['warmup']]
    bm=st.median(s['wallMs'] for s in nb)/1000; am=st.median(s['wallMs'] for s in na)/1000
    rb=st.median(s['maxRssBytes'] for s in nb)/1048576; ra=st.median(s['maxRssBytes'] for s in na)/1048576
    # every iteration (incl warmup) paired: result equal except work
    eq=all(strip(next(x for x in S if x['iteration']==i and x['side']=='before')['result'])==strip(next(x for x in S if x['iteration']==i and x['side']=='after')['result']) for i in range(4))
    wb={s['result']['work'] for s in S if s['side']=='before'}; wa={s['result']['work'] for s in S if s['side']=='after'}
    res=nb[0]['result']
    print(f"{w:>2} {bm:12.3f} {am:11.3f} {am/bm:6.3f} {res['states']:>6} {res['generated']:>9} {rb:9.1f} {ra:9.1f} {str(eq):>12} {sorted(wb)} {sorted(wa)}  B:{','.join('%.3f'%(s['wallMs']/1000) for s in nb)} A:{','.join('%.3f'%(s['wallMs']/1000) for s in na)} RSSB:{','.join('%.1f'%(s['maxRssBytes']/1048576) for s in nb)} RSSA:{','.join('%.1f'%(s['maxRssBytes']/1048576) for s in na)}")
    print('    result keys:', sorted(res.keys()), 'status', res['status'], 'depth', res.get('depth'), 'reason', res.get('reason'), 'violation', res.get('violation'))
print('controls (broken):')
for w in (1,2,4,8):
    b=next(c for c in r['controls'] if c['workers']==w and c['side']=='before'); a=next(c for c in r['controls'] if c['workers']==w and c['side']=='after')
    bx,ax=b['result'],a['result']
    diff=sorted(k for k in set(bx)|set(ax) if bx.get(k)!=ax.get(k))
    print(f"  w{w}: status {bx['status']}/{ax['status']} states {bx['states']}/{ax['states']} generated {bx['generated']}/{ax['generated']} depth {bx.get('depth')}/{ax.get('depth')} reason {bx.get('reason')}/{ax.get('reason')} work {bx['work']}/{ax['work']} differing keys {diff} except-work-equal {strip(bx)==strip(ax)} violation-equal {bx.get('violation')==ax.get('violation')} rss(MiB) {b['maxRssBytes']/1048576:.1f}/{a['maxRssBytes']/1048576:.1f}")
