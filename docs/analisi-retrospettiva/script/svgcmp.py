import sys,re,collections
def items(p):
    s=open(p).read()
    tags=re.findall(r'<(\w+)([^>]*)>',s)
    out=collections.Counter()
    imgs=[]
    for t,a in tags:
        if t=='image':
            m=re.search(r'href="data:[^;]+;base64,([^"]*)"',a); imgs.append(len(m.group(1)) if m else 0); out['image']+=1
        else: out[(t,a.strip())]+=1
    return out,imgs,s
o,oi,os_=items(sys.argv[1]); n,ni,ns=items(sys.argv[2])
only_o=o-n; only_n=n-o
kinds=lambda c: collections.Counter(k[0] if isinstance(k,tuple) else k for k in c.elements())
print("tag totali old",sum(o.values()),"new",sum(n.values()))
print("solo old per tipo",dict(kinds(only_o)),"solo new",dict(kinds(only_n)))
print("immagini old",oi,"new",ni)
if len(sys.argv)>3:
    for k in list(only_o)[:int(sys.argv[3])]: print("  -",k)
    for k in list(only_n)[:int(sys.argv[3])]: print("  +",k)
