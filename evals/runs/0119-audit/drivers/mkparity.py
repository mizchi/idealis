# usage: mkparity.py <snapshot dir with the native binary and idealis_core.wasm> <output dir>
import hashlib, json, os, shutil, sys
src, dst = sys.argv[1], sys.argv[2]
native = "idealis" + "-" + "core"
os.makedirs(dst)
shutil.copy(os.path.join(src, native), os.path.join(dst, native))
shutil.copy(os.path.join(src, "idealis_core.wasm"), os.path.join(dst, "core.wasm"))
def sha(path): return hashlib.sha256(open(path, "rb").read()).hexdigest()
manifest = {"binaries": [{"path": os.path.join(dst, name), "sha256": sha(os.path.join(dst, name))} for name in (native, "core.wasm")]}
json.dump(manifest, open(os.path.join(dst, "manifest.json"), "w"), indent=1)
print(json.dumps(manifest)[:300])
