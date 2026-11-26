import sys
from PIL import Image, ImageChops
a=Image.open(sys.argv[1]).convert('L'); b=Image.open(sys.argv[2]).convert('L')
if a.size!=b.size: print("dimensioni diverse",a.size,b.size); sys.exit()
d=ImageChops.difference(a,b); px=sum(1 for v in d.getdata() if v>40)
dark_a=sum(1 for v in a.getdata() if v<128); dark_b=sum(1 for v in b.getdata() if v<128)
print(f"pixel diversi(>40)={px} ({100*px/(a.size[0]*a.size[1]):.3f}%) scuri old={dark_a} new={dark_b}")
if len(sys.argv)>3:
    e=d.point(lambda v:255 if v>40 else 0); bb=e.getbbox(); print("zona",bb)
    vis=Image.merge('RGB',(a,b,b)); vis.save(sys.argv[3])
