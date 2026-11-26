import sys
n=int(sys.argv[1])
lines=["module chain {","  var counter: int","  pure def hop0(x: int): int = x + 1"]
for k in range(1,n+1):
    lines.append(f"  pure def hop{k}(x: int): int = hop{k-1}(x) + 1")
lines+=[
"  action init = counter' = 0",
f"  action step = counter' = hop{n}(counter) % 5",
"  val inv = counter >= 0",
"}"]
print("\n".join(lines))
