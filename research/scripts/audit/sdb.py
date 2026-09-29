import struct, json, os
R='/tmp/claude-1000/-home-twulfz-dev-wolluf/879b26ab-9465-45fd-b47b-2a469d1d893f/scratchpad/rejudge'
b=open('/mnt/e/Games/osu!/scores.db','rb').read(); o=0
def take(n):
    global o; v=b[o:o+n]; o+=n; return v
def I(): return struct.unpack('<i',take(4))[0]
def s():
    if take(1)[0]==0: return None
    n=sh=0
    while True:
        c=take(1)[0]; n|=(c&0x7f)<<sh; sh+=7
        if not c&0x80: break
    return take(n).decode('utf8','replace')
ver=I(); nb=I(); keys=set(); tot=0; modes={}
for _ in range(nb):
    md5=s(); n=I()
    for _ in range(n):
        mode=take(1)[0]; v=I(); bm=s(); pl=s(); rm=s(); take(12); take(4+2+1); mods=I(); s(); ts=struct.unpack('<q',take(8))[0]
        assert I()==-1; take(8)
        if mods & (1<<23): take(8)
        keys.add((bm,ts)); tot+=1
print('version',ver,'beatmaps',nb,'scores',tot, 'bytes left', len(b)-o)
d=json.load(open(R+'/batch_5000_wiki_tail_miss-kill_miss-pre=rehead200-v2best.json'))
ok=[x for x in d if 'error' not in x]
exp=lambda x: x['n_obj']+(x['n_ln'] if x['v2'] else 0)
under=[x for x in ok if x['n_hdr']<exp(x)]; over=[x for x in ok if x['n_hdr']>exp(x)]; comp=[x for x in ok if x['n_hdr']==exp(x)]
f=lambda L: sum((x['fn'].split('-')[0],x['ts']) in keys for x in L)
print('under',len(under),f(under),'over',len(over),f(over),'complete',len(comp),f(comp))
json.dump([x['fn'] for x in under], open('/tmp/claude-1000/-home-twulfz-dev-wolluf/879b26ab-9465-45fd-b47b-2a469d1d893f/scratchpad/audit/under.json','w'))
