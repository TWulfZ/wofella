"""Minimal osu!.db reader (stable). Layout: https://github.com/ppy/osu/wiki/Legacy-database-file-structure"""
import struct, json, sys


class R:
    def __init__(self, b):
        self.b, self.o = b, 0

    def take(self, n):
        v = self.b[self.o:self.o + n]
        self.o += n
        return v

    def u8(self): return self.take(1)[0]
    def i16(self): return struct.unpack('<h', self.take(2))[0]
    def i32(self): return struct.unpack('<i', self.take(4))[0]
    def i64(self): return struct.unpack('<q', self.take(8))[0]
    def f32(self): return struct.unpack('<f', self.take(4))[0]
    def f64(self): return struct.unpack('<d', self.take(8))[0]

    def uleb(self):
        r = s = 0
        while True:
            c = self.u8()
            r |= (c & 0x7f) << s
            s += 7
            if not c & 0x80:
                return r

    def s(self):
        t = self.u8()
        if t == 0:
            return None
        assert t == 0x0b, (t, self.o)
        return self.take(self.uleb()).decode('utf-8', 'replace')


def parse(path):
    r = R(open(path, 'rb').read())
    ver = r.i32(); r.i32(); r.u8(); r.i64(); r.s()
    n = r.i32()
    out = []
    for _ in range(n):
        if ver < 20191106:
            r.i32()
        artist = r.s(); r.s(); title = r.s(); r.s(); creator = r.s(); diff = r.s(); r.s()
        md5 = r.s(); fn = r.s()
        r.u8(); r.i16(); r.i16(); r.i16(); r.i64()
        ar, cs, hp, od = (r.f32() for _ in range(4))
        r.f64()
        for _m in range(4):
            k = r.i32()
            for _p in range(k):
                r.u8(); r.i32(); t = r.u8()
                r.f32() if t == 0x0c else r.f64()
        r.i32(); r.i32(); r.i32()
        r.take(17 * r.i32())
        r.i32(); bid = r.i32(); r.i32()
        r.take(4); r.i16(); r.f32()
        mode = r.u8()
        r.s(); r.s(); r.i16(); r.s(); r.u8(); r.i64(); r.u8()
        folder = r.s()
        r.i64(); r.take(5); r.i32(); r.u8()
        out.append(dict(md5=md5, folder=folder, file=fn, mode=mode, cs=cs, od=od, hp=hp,
                        artist=artist, title=title, diff=diff, creator=creator, bid=bid))
    return ver, out


if __name__ == '__main__':
    ver, maps = parse(sys.argv[1])
    print('version', ver, 'maps', len(maps), file=sys.stderr)
    json.dump({m['md5']: m for m in maps if m['md5']}, open(sys.argv[2], 'w'))
