"""Pembangkit kode LLVM untuk bahasa Nusa."""
import llvmlite.ir as ir

from . import ast
from .tipe import KOSONG, nama_tipe


class GalatCodegen(Exception):
    pass


class Pembangkit:
    def __init__(self, konteks, nama_file="<masukan>"):
        self.k = konteks
        self.nama_file = nama_file
        self.mod = ir.Module(name=nama_file)
        self.i1 = ir.IntType(1)
        self.i8 = ir.IntType(8)
        self.i32 = ir.IntType(32)
        self.i64 = ir.IntType(64)
        self.void = ir.VoidType()
        self.i8p = ir.IntType(8).as_pointer()
        self.tipe_bentuk = {}
        self.deklarasi = {}
        self.intrinsik = {}
        self._teks_cache = {}
        self._konter = 0
        self.vars = []
        self.loop = []
        self.rentang = []
        self.takaman = 0
        self.fn = None
        self.b = None

    # ---------- tipe ----------
    def tt(self, t):
        if t.kind == "int":
            return ir.IntType(t.bits)
        if t.kind == "float":
            return ir.FloatType() if t.bits == 32 else ir.DoubleType()
        if t.kind == "logika":
            return self.i1
        if t.kind == "teks":
            return self.i8p
        if t.kind == "kosong":
            return self.void
        if t.kind == "deret":
            return ir.ArrayType(self.tt(t.elem), t.n)
        if t.kind == "bentuk":
            if t.nama not in self.tipe_bentuk:
                self.tipe_bentuk[t.nama] = ir.LiteralStructType(
                    [self.tt(mt) for _, mt in t.medan])
            return self.tipe_bentuk[t.nama]
        if t.kind == "pilihan":
            return self.i32
        raise GalatCodegen(f"tipe {t.kind} belum didukung pembangkit kode")

    def konstanta_int(self, e):
        """Nilai bulat yang diketahui saat kompilasi, atau None."""
        if isinstance(e, ast.Angka):
            return e.nilai
        if isinstance(e, ast.Unari) and e.op == "-" and isinstance(e.operand, ast.Angka):
            return -e.operand.nilai
        if isinstance(e, ast.Panggil) and getattr(e, "nama_fn", None) == "panjang":
            t = e.argumen[0].tipe
            if t.kind == "deret":
                return t.n
        return None

    def rentang_var(self, nama):
        for r in reversed(self.rentang):
            if nama in r:
                return r[nama]
        return None

    def _nama(self, dasar="tmp"):
        self._konter += 1
        return f"{dasar}{self._konter}"

    # ---------- teks & pustaka ----------
    def teks_global(self, isi):
        kunci = isi
        if kunci in self._teks_cache:
            return self._teks_cache[kunci]
        data = isi.encode("utf-8") + b"\x00"
        gv = ir.GlobalVariable(self.mod, ir.ArrayType(self.i8, len(data)),
                               name=f"teks{len(self._teks_cache)}")
        gv.global_constant = True
        gv.linkage = "private"
        gv.initializer = ir.Constant(ir.ArrayType(self.i8, len(data)), bytearray(data))
        self._teks_cache[kunci] = gv
        return gv

    def _i8p(self, gv):
        return self.b.bitcast(gv, self.i8p)

    def tt_param(self, t):
        if t.kind == "deret":
            return self.tt(t).as_pointer()
        return self.tt(t)

    def pustaka(self, nama, ret, argumen, var_arg=False):
        if nama in self.deklarasi:
            return self.deklarasi[nama]
        f = ir.Function(self.mod, ir.FunctionType(ret, argumen, var_arg=var_arg), name=nama)
        self.deklarasi[nama] = f
        return f

    def _printf(self):
        return self.pustaka("printf", self.i32, [self.i8p], var_arg=True)

    def _strlen(self):
        return self.pustaka("strlen", self.i64, [self.i8p])

    def _strcmp(self):
        return self.pustaka("strcmp", self.i32, [self.i8p, self.i8p])

    def _buat_panik(self):
        fn = ir.Function(self.mod, ir.FunctionType(self.void, [self.i8p, self.i32]),
                         name="nusa_panik")
        fn.attributes.add("noreturn")
        b = ir.IRBuilder(fn.append_basic_block("masuk"))
        stderr = ir.GlobalVariable(self.mod, self.i8p, name="stderr")
        stderr.linkage = "external"
        fprintf = self.pustaka("fprintf", self.i32, [self.i8p, self.i8p], var_arg=True)
        fflush = self.pustaka("fflush", self.i32, [self.i8p])
        b.call(fflush, [ir.Constant(self.i8p, None)])
        b.call(fprintf, [b.load(stderr), b.bitcast(self.teks_global(
            "nusa: panik: %s (baris %d)\n"), self.i8p), fn.args[0], fn.args[1]])
        abort = self.pustaka("abort", self.void, [])
        abort.attributes.add("noreturn")
        b.call(abort, [])
        b.unreachable()
        self.deklarasi["nusa_panik"] = fn
        return fn

    def _panik(self, pelanggaran, pesan, baris):
        blok_panik = self.fn.append_basic_block(self._nama("panik"))
        blok_lanjut = self.fn.append_basic_block(self._nama("lanjut"))
        self.b.cbranch(pelanggaran, blok_panik, blok_lanjut)
        self.b = ir.IRBuilder(blok_panik)
        self.b.call(self.deklarasi["nusa_panik"],
                    [self._i8p(self.teks_global(pesan)), ir.Constant(self.i32, baris)])
        self.b.unreachable()
        self.b = ir.IRBuilder(blok_lanjut)

    def cek(self, pelanggaran, pesan, baris):
        if self.takaman:
            return
        self._panik(pelanggaran, pesan, baris)

    # ---------- deklarasi fungsi ----------
    def bangun(self):
        self._buat_panik()
        for fn in self.k.fungsi.values():
            if fn.nama == "utama":
                continue
            self.deklarasi[fn.nama] = ir.Function(
                self.mod, ir.FunctionType(self.tt(fn.tipe_hasil),
                                          [self.tt_param(t) for t in fn.tipe_param]),
                name=fn.nama)
        for fn in self.k.fungsi.values():
            if fn.nama == "utama":
                continue
            self.emit_fn(fn)
        self._emit_utama(self.k.fungsi["utama"])
        return str(self.mod)

    def _emit_utama(self, fn):
        f = ir.Function(self.mod, ir.FunctionType(self.i32, []), name="main")
        b = ir.IRBuilder(f.append_basic_block("masuk"))
        target = self.deklarasi.get("utama")
        if target is None:
            self.deklarasi["utama"] = ir.Function(
                self.mod, ir.FunctionType(self.tt(fn.tipe_hasil), []), name="utama")
            target = self.deklarasi["utama"]
            self.emit_fn(fn)
        hasil = b.call(target, [])
        if fn.tipe_hasil == KOSONG:
            b.ret(ir.Constant(self.i32, 0))
        elif fn.tipe_hasil.bits <= 32:
            b.ret(hasil)
        else:
            b.ret(b.trunc(hasil, self.i32))

    def emit_fn(self, fn):
        f = self.deklarasi[fn.nama]
        self.fn = f
        self.b = ir.IRBuilder(f.append_basic_block("masuk"))
        self.vars = [{}]
        self.loop = []
        self.rentang = [{}]
        self.takaman = 0
        for i, (p, tp) in enumerate(zip(fn.param, fn.tipe_param)):
            if tp.kind == "deret":
                self.vars[0][p.nama] = (f.args[i], tp)
            else:
                al = self.b.alloca(self.tt(tp), name=p.nama)
                self.b.store(f.args[i], al)
                self.vars[0][p.nama] = (al, tp)
        self.emit_stmts(fn.badan.isi)
        if not self.b.block.is_terminated:
            if fn.tipe_hasil == KOSONG:
                self.b.ret_void()
            else:
                self.b.unreachable()

    # ---------- scope ----------
    def dorong(self):
        self.vars.append({})

    def tarik(self):
        self.vars.pop()

    def cari(self, nama):
        for s in reversed(self.vars):
            if nama in s:
                return s[nama]
        raise GalatCodegen(f"variabel '{nama}' tidak ditemukan saat pembangkitan kode")

    def daftarkan(self, nama, ptr, tipe):
        self.vars[-1][nama] = (ptr, tipe)

    # ---------- pernyataan ----------
    def emit_stmts(self, daftar):
        for s in daftar:
            if self.b.block.is_terminated:
                return
            self.emit_stmt(s)

    def emit_blok(self, blok):
        self.dorong()
        self.emit_stmts(blok.isi)
        self.tarik()

    def emit_stmt(self, s):
        if isinstance(s, ast.Dekl):
            al = self.b.alloca(self.tt(s.tipe_ter), name=s.nama)
            v = self.nilai(s.nilai)
            self._simpan(al, v, s.tipe_ter)
            self.daftarkan(s.nama, al, s.tipe_ter)
        elif isinstance(s, ast.Rekah):
            ptr = self.alamat(s.sasaran)
            tipe = s.sasaran.tipe
            if s.op == "=":
                self._simpan(ptr, self.nilai(s.nilai), tipe)
            else:
                lama = self.b.load(ptr)
                kanan = self.nilai(s.nilai)
                hasil = self._arit(s.op[0], lama, kanan, tipe, s.baris)
                self._simpan(ptr, hasil, tipe)
        elif isinstance(s, ast.Jika):
            self._jika(s)
        elif isinstance(s, ast.Selama):
            self._selama(s)
        elif isinstance(s, ast.Untuk):
            self._untuk(s)
        elif isinstance(s, ast.Ulang):
            self._ulang(s)
        elif isinstance(s, ast.Henti):
            self.b.branch(self.loop[-1][1])
        elif isinstance(s, ast.Lanjut):
            self.b.branch(self.loop[-1][0])
        elif isinstance(s, ast.Kembali):
            if s.nilai is None:
                self.b.ret_void()
            else:
                v = self.nilai(s.nilai)
                self.b.ret(self.muat(v, s.nilai.tipe))
        elif isinstance(s, ast.Periksa):
            v = self.nilai(s.syarat)
            pesan = s.pesan or "periksa gagal"
            self.cek(self.b.not_(v), pesan, s.baris)
        elif isinstance(s, ast.Takaman):
            self.takaman += 1
            self.emit_blok(s.badan)
            self.takaman -= 1
        elif isinstance(s, ast.Cocok):
            self._cocok(s)
        elif isinstance(s, ast.EkspresiStmt):
            self.nilai(s.ekspresi)
        elif isinstance(s, ast.Blok):
            self.emit_blok(s)
        else:
            raise GalatCodegen(f"pernyataan tidak didukung: {type(s).__name__}")

    def _jika(self, s):
        cond = self.nilai(s.syarat)
        blok_maka = self.fn.append_basic_block(self._nama("maka"))
        blok_lain = self.fn.append_basic_block(self._nama("lain"))
        blok_akhir = self.fn.append_basic_block(self._nama("akhir"))
        self.b.cbranch(cond, blok_maka, blok_lain)
        self.b = ir.IRBuilder(blok_maka)
        self.emit_blok(s.maka)
        if not self.b.block.is_terminated:
            self.b.branch(blok_akhir)
        self.b = ir.IRBuilder(blok_lain)
        if isinstance(s.lainnya, ast.Blok):
            self.emit_blok(s.lainnya)
        elif isinstance(s.lainnya, ast.Jika):
            self._jika(s.lainnya)
        if not self.b.block.is_terminated:
            self.b.branch(blok_akhir)
        self.b = ir.IRBuilder(blok_akhir)

    def _selama(self, s):
        blok_uji = self.fn.append_basic_block(self._nama("uji"))
        blok_badan = self.fn.append_basic_block(self._nama("badan"))
        blok_akhir = self.fn.append_basic_block(self._nama("akhir"))
        self.b.branch(blok_uji)
        self.b = ir.IRBuilder(blok_uji)
        self.b.cbranch(self.nilai(s.syarat), blok_badan, blok_akhir)
        self.b = ir.IRBuilder(blok_badan)
        self.loop.append((blok_uji, blok_akhir))
        self.emit_blok(s.badan)
        self.loop.pop()
        if not self.b.block.is_terminated:
            self.b.branch(blok_uji)
        self.b = ir.IRBuilder(blok_akhir)

    def _ulang(self, s):
        blok_badan = self.fn.append_basic_block(self._nama("badan"))
        blok_akhir = self.fn.append_basic_block(self._nama("akhir"))
        self.b.branch(blok_badan)
        self.b = ir.IRBuilder(blok_badan)
        self.loop.append((blok_badan, blok_akhir))
        self.emit_blok(s.badan)
        self.loop.pop()
        if not self.b.block.is_terminated:
            self.b.branch(blok_badan)
        self.b = ir.IRBuilder(blok_akhir)

    def _untuk(self, s):
        tipe = s.tipe_var
        al = self.b.alloca(self.tt(tipe), name=s.nama)
        self._simpan(al, self.nilai(s.awal), tipe)
        batas = self.nilai(s.akhir)
        blok_uji = self.fn.append_basic_block(self._nama("uji"))
        blok_badan = self.fn.append_basic_block(self._nama("badan"))
        blok_akhir = self.fn.append_basic_block(self._nama("akhir"))
        self.b.branch(blok_uji)
        self.b = ir.IRBuilder(blok_uji)
        kini = self.b.load(al)
        pred = "<=" if s.inklusif else "<"
        cond = self._banding(pred, kini, batas, tipe)
        self.b.cbranch(cond, blok_badan, blok_akhir)
        self.b = ir.IRBuilder(blok_badan)
        self.dorong()
        self.daftarkan(s.nama, al, tipe)
        awal_tetap = self.konstanta_int(s.awal)
        akhir_tetap = self.konstanta_int(s.akhir)
        if awal_tetap is not None and akhir_tetap is not None:
            batas = akhir_tetap if s.inklusif else akhir_tetap - 1
            self.rentang.append({s.nama: (awal_tetap, batas)})
        else:
            self.rentang.append({})
        self.loop.append((blok_uji, blok_akhir))
        self.emit_stmts(s.badan.isi)
        self.loop.pop()
        self.rentang.pop()
        self.tarik()
        if not self.b.block.is_terminated:
            satu = ir.Constant(self.tt(tipe), 1)
            baru = self._arit("+", self.b.load(al), satu, tipe, s.baris)
            self.b.store(baru, al)
            self.b.branch(blok_uji)
        self.b = ir.IRBuilder(blok_akhir)

    def _cocok(self, s):
        v = self.nilai(s.nilai)
        tn = s.tipe_nilai
        blok_akhir = self.fn.append_basic_block(self._nama("cocok"))
        daftar = list(s.kasus)
        if s.lainnya is not None:
            daftar.append((ast.Pola(baris=s.baris, jenis="lainnya"), s.lainnya))
        for pola, blok in daftar:
            if pola.jenis == "lainnya":
                self.emit_blok(blok)
                if not self.b.block.is_terminated:
                    self.b.branch(blok_akhir)
                self.b = ir.IRBuilder(blok_akhir)
                return
            blok_arm = self.fn.append_basic_block(self._nama("kasus"))
            blok_uji = self.fn.append_basic_block(self._nama("uji"))
            if tn.kind == "pilihan":
                nilai_pola = ir.Constant(self.i32, pola.indeks)
            else:
                nilai_pola = ir.Constant(self.tt(tn), pola.nilai)
            self.b.cbranch(self.b.icmp_signed("==", v, nilai_pola), blok_arm, blok_uji)
            self.b = ir.IRBuilder(blok_arm)
            self.emit_blok(blok)
            if not self.b.block.is_terminated:
                self.b.branch(blok_akhir)
            self.b = ir.IRBuilder(blok_uji)
        self.b.branch(blok_akhir)
        self.b = ir.IRBuilder(blok_akhir)

    # ---------- nilai & alamat ----------
    def _simpan(self, ptr, nilai, tipe):
        if tipe.kind == "deret":
            self.b.store(self.b.load(nilai), ptr)
        else:
            self.b.store(nilai, ptr)

    def muat(self, nilai, tipe):
        if tipe.kind == "deret":
            return self.b.load(nilai)
        return nilai

    def paksa_ptr(self, nilai, tipe):
        al = self.b.alloca(self.tt(tipe))
        self._simpan(al, nilai, tipe)
        return al

    def alamat(self, e):
        if isinstance(e, ast.Nama):
            return self.cari(e.nama)[0]
        if isinstance(e, ast.Indeks):
            tbase = e.dasar.tipe
            dasar_ptr = self.alamat(e.dasar)
            if dasar_ptr is None:
                dasar_ptr = self.paksa_ptr(self.nilai(e.dasar), tbase)
            idx = self.nilai(e.indeks)
            idx64 = self._ke_i64(idx, e.indeks.tipe)
            n = ir.Constant(self.i64, tbase.n)
            pasti_aman = False
            if isinstance(e.indeks, ast.Nama):
                r = self.rentang_var(e.indeks.nama)
                pasti_aman = r is not None and r[0] >= 0 and r[1] < tbase.n
            else:
                k = self.konstanta_int(e.indeks)
                pasti_aman = k is not None and 0 <= k < tbase.n
            if not self.takaman and not pasti_aman:
                if e.indeks.tipe.kind == "int" and e.indeks.tipe.signed:
                    neg = self.b.icmp_signed("<", idx64, ir.Constant(self.i64, 0))
                    besar = self.b.icmp_unsigned(">=", idx64, n)
                    self.cek(self.b.or_(neg, besar),
                             f"indeks deret di luar batas 0..{tbase.n - 1}", e.baris)
                else:
                    self.cek(self.b.icmp_unsigned(">=", idx64, n),
                             f"indeks deret di luar batas 0..{tbase.n - 1}", e.baris)
            return self.b.gep(dasar_ptr, [ir.Constant(self.i32, 0), idx64])
        if isinstance(e, ast.Medan):
            tbase = e.dasar.tipe
            dasar_ptr = self.alamat(e.dasar)
            if dasar_ptr is None:
                dasar_ptr = self.paksa_ptr(self.nilai(e.dasar), tbase)
            for i, (mn, _) in enumerate(tbase.medan):
                if mn == e.nama:
                    return self.b.gep(dasar_ptr, [ir.Constant(self.i32, 0),
                                                  ir.Constant(self.i32, i)])
            raise GalatCodegen(f"medan '{e.nama}' tidak ditemukan")
        return None

    def nilai(self, e):
        tipe = e.tipe
        if isinstance(e, ast.Angka):
            if hasattr(e, "nilai_pecahan"):
                return ir.Constant(self.tt(tipe), e.nilai_pecahan)
            return ir.Constant(self.tt(tipe), e.nilai)
        if isinstance(e, ast.Pecahan):
            return ir.Constant(self.tt(tipe), e.nilai)
        if isinstance(e, ast.Logika):
            return ir.Constant(self.i1, 1 if e.nilai else 0)
        if isinstance(e, ast.Teks):
            return self._i8p(self.teks_global(e.nilai))
        if isinstance(e, (ast.Nama, ast.Indeks, ast.Medan)):
            if isinstance(e, ast.Medan) and getattr(e, "konstanta_pilihan", False):
                return ir.Constant(self.i32, e.indeks_varian)
            ptr = self.alamat(e)
            if ptr is None:
                raise GalatCodegen(f"ekspresi {type(e).__name__} tidak memiliki alamat")
            if tipe.kind == "deret":
                return ptr
            return self.b.load(ptr)
        if isinstance(e, ast.Deret):
            al = self.b.alloca(self.tt(tipe))
            for i, x in enumerate(e.elemen):
                v = self.nilai(x)
                self.b.store(v, self.b.gep(al, [ir.Constant(self.i32, 0), ir.Constant(self.i32, i)]))
            return al
        if isinstance(e, ast.Struktur):
            t = tipe
            nilai = ir.Constant(self.tt(t), None)
            for i, (mn, mt) in enumerate(t.medan):
                v = self.nilai(e.medan_ter[mn])
                if mt.kind == "deret":
                    v = self.muat(v, mt)
                nilai = self.b.insert_value(nilai, v, i)
            return nilai
        if isinstance(e, ast.Biner):
            return self._biner(e)
        if isinstance(e, ast.Unari):
            return self._unari(e)
        if isinstance(e, ast.Sbg):
            return self._sbg(e)
        if isinstance(e, ast.Panggil):
            return self._panggil(e)
        raise GalatCodegen(f"ekspresi tidak didukung: {type(e).__name__}")

    # ---------- operator ----------
    def _arit(self, op, a, b, tipe, baris):
        if tipe.kind == "float":
            return {"+": self.b.fadd, "-": self.b.fsub,
                    "*": self.b.fmul, "/": self.b.fdiv,
                    "%": self.b.frem}[op](a, b)
        if tipe.kind != "int":
            raise GalatCodegen(f"aritmetika untuk {tipe.kind} tidak didukung")
        if op in ("/", "%"):
            return self._bagi(op, a, b, tipe, baris)
        if self.takaman:
            return {"+": self.b.add, "-": self.b.sub, "*": self.b.mul}[op](a, b)
        dasar = {"+": "add", "-": "sub", "*": "mul"}[op]
        awalan = "s" if tipe.signed else "u"
        fn = self._intrinsik(f"llvm.{awalan}{dasar}.with.overflow.i{tipe.bits}", tipe)
        dua = self.b.call(fn, [a, b])
        hasil = self.b.extract_value(dua, 0)
        luap = self.b.extract_value(dua, 1)
        self.cek(luap, f"luapan aritmetika {op} pada {nama_tipe(tipe)}", baris)
        return hasil

    def _bagi(self, op, a, b, tipe, baris):
        if tipe.signed:
            self.cek(self.b.icmp_signed("==", b, ir.Constant(self.tt(tipe), 0)),
                     f"pembagian dengan nol pada {nama_tipe(tipe)}", baris)
            minv = -(1 << (tipe.bits - 1))
            self.cek(self.b.and_(self.b.icmp_signed("==", a, ir.Constant(self.tt(tipe), minv)),
                                 self.b.icmp_signed("==", b, ir.Constant(self.tt(tipe), -1))),
                     "luapan pembagian minimum dengan -1", baris)
            return (self.b.sdiv if op == "/" else self.b.srem)(a, b)
        self.cek(self.b.icmp_unsigned("==", b, ir.Constant(self.tt(tipe), 0)),
                 "pembagian dengan nol", baris)
        return (self.b.udiv if op == "/" else self.b.urem)(a, b)

    def _intrinsik(self, nama, tipe):
        if nama in self.intrinsik:
            return self.intrinsik[nama]
        t = self.tt(tipe)
        fn = ir.Function(self.mod, ir.FunctionType(ir.LiteralStructType([t, self.i1]), [t, t]),
                         name=nama)
        self.intrinsik[nama] = fn
        return fn

    def _banding(self, op, a, b, tipe):
        if tipe.kind == "float":
            peta = {"==": "==", "!=": "!=", "<": "<", "<=": "<=", ">": ">", ">=": ">="}
            return self.b.fcmp_ordered(peta[op], a, b)
        if tipe.kind == "teks":
            cmp = self.b.call(self._strcmp(), [a, b])
            return self.b.icmp_signed({"==": "==", "!=": "!=", "<": "<", "<=": "<=",
                                       ">": ">", ">=": ">="}[op], cmp, ir.Constant(self.i32, 0))
        peta = {"==": "==", "!=": "!=", "<": "<", "<=": "<=", ">": ">", ">=": ">="}
        if tipe.kind == "int" and not tipe.signed:
            return self.b.icmp_unsigned(peta[op], a, b)
        return self.b.icmp_signed(peta[op], a, b)

    def _biner(self, e):
        op = e.op
        tipe = e.tipe
        if op in ("dan", "atau", "&&", "||"):
            return self._logika(e)
        a = self.nilai(e.kiri)
        b = self.nilai(e.kanan)
        if op in ("+", "-", "*", "/", "%"):
            return self._arit(op, a, b, tipe if tipe.kind in ("int", "float") else e.kiri.tipe, e.baris)
        if op in ("==", "!=", "<", "<=", ">", ">="):
            return self._banding(op, a, b, e.kiri.tipe)
        if op in ("&", "|", "^"):
            return {"&": self.b.and_, "|": self.b.or_, "^": self.b.xor}[op](a, b)
        if op in ("<<", ">>"):
            tk = e.kiri.tipe
            if not self.takaman:
                if e.kanan.tipe.kind == "int" and e.kanan.tipe.signed:
                    self.cek(self.b.or_(self.b.icmp_signed("<", b, ir.Constant(self.tt(e.kanan.tipe), 0)),
                                        self.b.icmp_unsigned(">=", b, ir.Constant(self.tt(e.kanan.tipe), tk.bits))),
                             f"geser melebihi {tk.bits} bit", e.baris)
                else:
                    self.cek(self.b.icmp_unsigned(">=", b, ir.Constant(self.tt(e.kanan.tipe), tk.bits)),
                             f"geser melebihi {tk.bits} bit", e.baris)
            return (self.b.shl if op == "<<" else
                    (self.b.ashr if tk.signed else self.b.lshr))(a, b)
        raise GalatCodegen(f"operator '{op}' tidak didukung")

    def _logika(self, e):
        blok_kanan = self.fn.append_basic_block(self._nama("kanan"))
        blok_lanjut = self.fn.append_basic_block(self._nama("gabung"))
        kiri = self.nilai(e.kiri)
        blok_kiri = self.b.block
        dan = e.op in ("dan", "&&")
        if dan:
            self.b.cbranch(kiri, blok_kanan, blok_lanjut)
        else:
            self.b.cbranch(kiri, blok_lanjut, blok_kanan)
        self.b = ir.IRBuilder(blok_kanan)
        kanan = self.nilai(e.kanan)
        blok_kanan_akhir = self.b.block
        self.b.branch(blok_lanjut)
        self.b = ir.IRBuilder(blok_lanjut)
        phi = self.b.phi(self.i1)
        short = ir.Constant(self.i1, 0 if dan else 1)
        phi.add_incoming(short, blok_kiri)
        phi.add_incoming(kanan, blok_kanan_akhir)
        return phi

    def _unari(self, e):
        tipe = e.tipe
        v = self.nilai(e.operand)
        if e.op == "!":
            return self.b.not_(v)
        if e.op == "-":
            if tipe.kind == "float":
                return self.b.fneg(v)
            if not self.takaman:
                self.cek(self.b.icmp_signed("==", v, ir.Constant(self.tt(tipe),
                                                                 -(1 << (tipe.bits - 1)))),
                         f"negasi meluap pada {nama_tipe(tipe)}", e.baris)
            return self.b.sub(ir.Constant(self.tt(tipe), 0), v)
        if e.op == "~":
            return self.b.not_(v)
        raise GalatCodegen(f"operator unari '{e.op}' tidak didukung")

    def _sbg(self, e):
        asal = e.nilai.tipe
        tujuan = e.tipe
        v = self.nilai(e.nilai)
        if asal == tujuan:
            return v
        if asal.kind == "logika" and tujuan.kind == "int":
            return self.b.zext(v, self.tt(tujuan))
        if asal.kind == "int" and tujuan.kind == "int":
            if tujuan.bits == asal.bits:
                return v
            if tujuan.bits < asal.bits:
                return self.b.trunc(v, self.tt(tujuan))
            return self.b.sext(v, self.tt(tujuan)) if asal.signed else self.b.zext(v, self.tt(tujuan))
        if asal.kind == "int" and tujuan.kind == "float":
            return self.b.sitofp(v, self.tt(tujuan)) if asal.signed else self.b.uitofp(v, self.tt(tujuan))
        if asal.kind == "float" and tujuan.kind == "float":
            return self.b.fptrunc(v, self.tt(tujuan)) if tujuan.bits < asal.bits else self.b.fpext(v, self.tt(tujuan))
        if asal.kind == "float" and tujuan.kind == "int":
            if not self.takaman:
                bawah = -(1 << (tujuan.bits - 1)) if tujuan.signed else 0
                atas = (1 << (tujuan.bits - 1)) if tujuan.signed else (1 << tujuan.bits)
                dv = self.tt(asal)
                salah = self.b.or_(self.b.fcmp_ordered("<", v, ir.Constant(dv, float(bawah))),
                                   self.b.fcmp_ordered(">=", v, ir.Constant(dv, float(atas))))
                self.cek(salah, f"nilai pecahan di luar rentang {nama_tipe(tujuan)}", e.baris)
            return self.b.fptosi(v, self.tt(tujuan)) if tujuan.signed else self.b.fptoui(v, self.tt(tujuan))
        raise GalatCodegen(f"konversi {nama_tipe(asal)} ke {nama_tipe(tujuan)} belum didukung")

    def _ke_i64(self, v, tipe):
        if tipe.kind != "int":
            return v
        if tipe.bits == 64:
            return v
        return self.b.sext(v, self.i64) if tipe.signed else self.b.zext(v, self.i64)

    def _panggil(self, e):
        nama = e.nama_fn
        if nama == "cetak":
            return self._cetak(e)
        if nama == "panjang":
            t = e.argumen[0].tipe
            if t.kind == "deret":
                return ir.Constant(self.i64, t.n)
            v = self.nilai(e.argumen[0])
            return self.b.call(self._strlen(), [v])
        fn = self.deklarasi[nama]
        args = [self.nilai(a) for a in e.argumen]
        hasil = self.b.call(fn, args)
        if isinstance(fn.function_type.return_type, ir.VoidType):
            return None
        if e.tipe is not None and e.tipe.kind == "deret":
            al = self.b.alloca(self.tt(e.tipe))
            self.b.store(hasil, al)
            return al
        return hasil

    def _cetak(self, e):
        fmt = []
        args = []
        for i, arg in enumerate(e.argumen):
            if i:
                fmt.append(" ")
            t = arg.tipe
            if t.kind == "int":
                v = self.nilai(arg)
                if t.bits < 32:
                    v = self.b.sext(v, self.i32) if t.signed else self.b.zext(v, self.i32)
                fmt.append(("%lld" if t.bits == 64 else "%d") if t.signed
                           else ("%llu" if t.bits == 64 else "%u"))
                args.append(v)
            elif t.kind == "float":
                v = self.nilai(arg)
                if t.bits == 32:
                    v = self.b.fpext(v, ir.DoubleType())
                fmt.append("%g")
                args.append(v)
            elif t.kind == "logika":
                v = self.nilai(arg)
                fmt.append("%s")
                args.append(self.b.select(v, self._i8p(self.teks_global("benar")),
                                          self._i8p(self.teks_global("salah"))))
            elif t.kind == "teks":
                fmt.append("%s")
                args.append(self.nilai(arg))
            elif t.kind == "pilihan":
                v = self.nilai(arg)
                hasil = self._i8p(self.teks_global("?"))
                for j in reversed(range(len(t.varian))):
                    hasil = self.b.select(self.b.icmp_signed("==", v, ir.Constant(self.i32, j)),
                                          self._i8p(self.teks_global(t.varian[j])), hasil)
                fmt.append("%s")
                args.append(hasil)
        fmt.append("\n")
        self.b.call(self._printf(), [self._i8p(self.teks_global("".join(fmt)))] + args)
        return None


def bangun_ir(konteks, nama_file="<masukan>"):
    return Pembangkit(konteks, nama_file).bangun()
