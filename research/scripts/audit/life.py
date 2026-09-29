import json, struct, os, collections
R="/mnt/e/Games/osu!/Data/r"
L=json.load(open('/tmp/claude-1000/-home-twulfz-dev-wolluf/879b26ab-9465-45fd-b47b-2a469d1d893f/scratchpad/audit/under.json'))
def rd(p):
    b=open(p,'rb').read(); o=0
    def take(n):
        nonlocal o; v=b[o:o+n]; o+=n; return v
    def s():
        if take(1)[0]==0: return None
        n=sh=0
        while True:
            c=take(1)[0]; n|=(c&0x7f)<<sh; sh+=7
            if not c&0x80: break
        return take(n).decode()
    take(5); s();s();s(); take(12); take(7); mods=struct.unpack('<i',take(4))[0]; lg=s(); ts=struct.unpack('<q',take(8))[0]
    return mods, lg
vals=[]; nog=0; mc=collections.Counter()
for fn in L:
    mods, lg = rd(os.path.join(R,fn)); mc[mods]+=1
    if not lg: nog+=1; continue
    last=[p for p in lg.split(',') if p][-1].split('|')
    vals.append(float(last[1]))
print('n',len(L),'no graph',nog,'with graph',len(vals),'end<=0.01',sum(v<=0.01 for v in vals), 'mods',mc)
print(sorted(vals)[-10:])
