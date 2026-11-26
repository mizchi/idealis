import subprocess, sys, re
base, tree, orig_patch, out = sys.argv[1:5]
files = re.findall(r'^diff -ruN a/(\S+) b/', open(orig_patch).read(), re.M)
res = []
for f in files:
    p = subprocess.run(['diff','-uN','a/'+f,'b/'+f], cwd=base, capture_output=True, text=True)
    lines = p.stdout.split('\n')
    lines[0] = re.sub(r'\t.*','',lines[0]); lines[1] = re.sub(r'\t.*','',lines[1])
    res.append('diff -ruN a/%s b/%s\n' % (f, f) + '\n'.join(lines))
open(out,'w').write(''.join(res))
