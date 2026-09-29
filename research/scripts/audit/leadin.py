import os, lzma, struct, random, collections
R="/mnt/e/Games/osu!/Data/r"
def uleb(f):
    r=s=0
    while True:
        b=f.read(1)[0]; r|=(b&0x7f)<<s; s+=7
        if not b&0x80: return r
def st(f):
    if f.read(1)[0]==0x0b: return f.read(uleb(f)).decode('utf8','replace')
    return None
files=[x for x in os.listdir(R) if x.endswith('.osr')]
random.seed(1); samp=random.sample(files, 200)
c=collections.Counter(); modes=collections.Counter()
for fn in samp:
    with open(os.path.join(R,fn),'rb') as f:
        mode=f.read(1)[0]; ver=struct.unpack('<i',f.read(4))[0]
        st(f);st(f);st(f); f.read(2*6+4+2+1+4); st(f); f.read(8); n=struct.unpack('<i',f.read(4))[0]
        data=f.read(n)
    modes[mode]+=1
    if mode!=3: continue
    try: s=lzma.decompress(data,format=lzma.FORMAT_ALONE).decode()
    except Exception as e: c['err']+=1; continue
    fr=s.split(',')[:3]
    c[tuple(x.split('|')[0]+'@'+x.split('|')[1]+','+x.split('|')[2] for x in fr[:2])]+=1
print(modes); print(c.most_common(10))
print('---')
k=0
for fn in samp:
    with open(os.path.join(R,fn),'rb') as f:
        mode=f.read(1)[0]; ver=struct.unpack('<i',f.read(4))[0]
        st(f);st(f);st(f); f.read(2*6+4+2+1+4); st(f); f.read(8); n=struct.unpack('<i',f.read(4))[0]
        data=f.read(n)
    if mode!=3: continue
    try: s=lzma.decompress(data,format=lzma.FORMAT_ALONE).decode()
    except: continue
    fr=s.split(',')
    if fr[1].split('|')[0]!='-1':
        # first frame with keys pressed
        t=0; first=None
        for i,x in enumerate(fr):
            p=x.split('|')
            if len(p)<4 or p[0]=='-12345': continue
            t+=int(p[0])
            if i>=2 and float(p[1])>0 and first is None: first=(i,t)
        print(fn[:12], fr[:4], 'first key frame', first); k+=1
        if k>6: break
