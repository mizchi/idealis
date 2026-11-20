import json, statistics as st, sys
E='<scratch>/eval-0073/runs'
def load(name): return json.load(open(f'{E}/{name}/result.json'))
def med(v): return st.median(v)
def table(name, key, label, rss=None):
    r=load(name); rows=[]
    print(f'== {name} ({label})')
    print(f"{'workers':>7} {'before med s':>12} {'after med s':>12} {'ratio':>7} {'states':>6} {'generated':>9} {'work b/a':>25} work= {'RSS b/a MiB':>16} samples before/after (s)")
    workers=sorted({s['workers'] for s in r['samples']})
    for w in workers:
        S=[s for s in r['samples'] if s['workers']==w and not s['warmup']]
        b=[s for s in S if s['side']=='before']; a=[s for s in S if s['side']=='after']
        bm=med([s[key] for s in b])/1000; am=med([s[key] for s in a])/1000
        st_b={(s['result']['states'],s['result']['generated'],s['result']['status'],s['result'].get('depth')) for s in b}
        st_a={(s['result']['states'],s['result']['generated'],s['result']['status'],s['result'].get('depth')) for s in a}
        wb={s['result']['work'] for s in b}; wa={s['result']['work'] for s in a}
        allS=[s for s in r['samples'] if s['workers']==w]
        # full result equality incl warmup, per iteration pair
        eq=all(next(x for x in allS if x['iteration']==i and x['side']=='before')['result']==next(x for x in allS if x['iteration']==i and x['side']=='after')['result'] for i in range(4))
        rb=ra=None
        for k in ('maxRssBytes','peakRssBytes'):
            if k in b[0]:
                rb=med([s[k] for s in b])/1048576; ra=med([s[k] for s in a])/1048576
        print(f"{w:>7} {bm:12.3f} {am:12.3f} {am/bm:7.3f} {b[0]['result']['states']:>6} {b[0]['result']['generated']:>9} {str(sorted(wb))+'/'+str(sorted(wa)):>25} {'yes' if wb==wa else 'NO'} full-result-equal={eq} states-same={st_b==st_a} " + (f"{rb:7.1f}/{ra:7.1f}" if rb is not None else '') + '  B:'+','.join(f'{s[key]/1000:.3f}' for s in b)+' A:'+','.join(f'{s[key]/1000:.3f}' for s in a))
    print('controls (broken invariant):')
    for w in sorted({c['workers'] for c in r['controls']}):
        b=next(c for c in r['controls'] if c['workers']==w and c['side']=='before'); a=next(c for c in r['controls'] if c['workers']==w and c['side']=='after')
        print(f"  w{w}: before {b['result']['status']} states={b['result']['states']} gen={b['result']['generated']} work={b['result']['work']} | after {a['result']['status']} states={a['result']['states']} gen={a['result']['generated']} work={a['result']['work']} | full result equal incl. trace: {b['result']==a['result']}")
table('0073-r1-nondet-native','checkMs','check-safety-parallel request time, compile excluded; compile+check in one process')
table('0073-r1-nondet-native-rss','wallMs','node wall of /usr/bin/time -l child, check only on precompiled program; RSS = time -l maximum resident set size')
table('0073-r1-nondet-native-capped','wallMs','node wall of capped.py (poll 0.2 s, quantised); RSS = capped.py peak_rss_kb')
