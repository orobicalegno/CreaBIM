import re,hashlib
def norm_points(t,a):
    m=re.search(r'points="([^"]*)"',a)
    if not m: return t+a
    pts=m.group(1).split()
    if t=='polyline':
        pts=min(pts,pts[::-1])
    elif t=='polygon':
        rots=[pts[i:]+pts[:i] for i in range(len(pts))]; rots+= [r[::-1] for r in rots]
        pts=min(rots)
    return t+a.replace(m.group(0),'points="'+' '.join(pts)+'"')
def canon(p):
    s=open(p).read()
    vec=[];img=[]
    for t,a in re.findall(r'<(\w+)([^>]*)>',s):
        if t=='image': img.append(hashlib.sha1(a.encode()).hexdigest()[:8])
        else: vec.append(norm_points(t,a))
    vec.sort()
    return hashlib.sha1("\n".join(vec).encode()).hexdigest()[:10], hashlib.sha1("".join(sorted(img)).encode()).hexdigest()[:6], vec
