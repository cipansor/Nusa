"""Verifikasi kontrak fungsi Nusa dengan Z3 (untuk sistem kritis).

Model: bilangan bulat sebagai Int matematis ditambah kewajiban rentang,
sehingga luapan mesin terbukti tidak terjadi. Pembagian bilangan bulat
sengaja tidak dimodelkan (pembulatan mesin berbeda dari aritmetika eksak);
gunakan pecahan atau bantu dengan 'periksa'.
"""
import os

import z3

from . import ast
from .parser import parse
from .periksa import periksa
from .tipe import KOSONG, hitung_rentang, nama_tipe

BATAS_OBLIGASI = 2000


class TidakDidukung(Exception):
    def __init__(self, alasan, baris=0):
        self.alasan = alasan
        self.baris = baris
        super().__init__(alasan)


class Status:
    __slots__ = ("path", "fakta", "env")

    def __init__(self, path=None, fakta=None, env=None):
        self.path = list(path or [])
        self.fakta = list(fakta or [])
        self.env = dict(env or {})

    def salinan(self):
        return Status(self.path, self.fakta, self.env)


class Hasil:
    def __init__(self):
        self.keluar = []       # (path, fakta, nilai_z3|None, baris)
        self.obligasi = []     # (path, fakta, kondisi_z3, keterangan, baris)
        self.unsupported = []  # (baris, alasan)


class Verifikator:
    def __init__(self, konteks):
        self.k = konteks
        self.tumpukan = []
        self.hasil = None
        self.fn = None
        self._konter = 0
        self.spec = 0

    # ---------- masuk ----------
    def periksa_fn(self, fn):
        self.fn = fn
        self.hasil = Hasil()
        st = Status()
        for p, tp in zip(fn.param, fn.tipe_param):
            v = self._var(f"{fn.nama}.{p.nama}", tp, p)
            st.env[p.nama] = (v, tp)
            lo, hi = hitung_rentang(tp)
            if lo is not None:
                st.fakta.append(v >= lo)
                st.fakta.append(v <= hi)
        self.spec += 1
        for sy in fn.syarat:
            st.fakta.append(self.eval_bool(sy, st))
        self.spec -= 1
        self.tumpukan.append(fn.nama)
        try:
            states = self.jalankan(fn.badan.isi, [st])
        except TidakDidukung as e:
            self.hasil.unsupported.append((e.baris or fn.baris, e.alasan))
            states = []
        finally:
            self.tumpukan.pop()
        for stx in states:
            self.hasil.keluar.append((stx.path, stx.fakta, None, fn.baris))
        if fn.jamin and not self.hasil.keluar:
            self.hasil.unsupported.append(
                (fn.baris, "tidak ada jalur kembali yang dapat dianalisis; "
                           "jamin tidak dapat diperiksa"))
        else:
            self._periksa_pascakondisi(fn)
        return self.hasil

    def _periksa_pascakondisi(self, fn):
        for path, fakta, nilai, _baris in self.hasil.keluar:
            st = Status(path, fakta)
            for p, tp in zip(fn.param, fn.tipe_param):
                st.env[p.nama] = (self._var(f"{fn.nama}.{p.nama}", tp, p), tp)
            st.env["hasil"] = (nilai if nilai is not None else z3.IntVal(0), fn.tipe_hasil)
            for jm in fn.jamin:
                if fn.tipe_hasil == KOSONG:
                    raise TidakDidukung("jamin pada fungsi tanpa nilai kembali", jm.baris)
                self.spec += 1
                kondisi = self.eval_bool(jm, st)
                self.spec -= 1
                self.hasil.obligasi.append(
                    (path, fakta, kondisi, "jamin (pascakondisi)", jm.baris))

    # ---------- nilai z3 ----------
    def _var(self, nama, tp, simpul):
        nama = nama.replace(".", "_")
        if tp.kind == "int":
            return z3.Int(nama)
        if tp.kind == "float":
            return z3.Real(nama)
        if tp.kind == "logika":
            return z3.Bool(nama)
        raise TidakDidukung(f"verifikasi belum mendukung tipe {nama_tipe(tp)}", simpul.baris)

    def _konstanta(self, e):
        if isinstance(e, ast.Angka):
            return z3.IntVal(e.nilai)
        if isinstance(e, ast.Pecahan):
            return z3.RealVal(e.nilai)
        if isinstance(e, ast.Logika):
            return z3.BoolVal(e.nilai)
        return None

    # ---------- evaluasi ----------
    def eval_bruto(self, e, st):
        k = self._konstanta(e)
        if k is not None:
            return k
        if isinstance(e, ast.Nama):
            if e.nama not in st.env:
                raise TidakDidukung(f"nama '{e.nama}' tidak dapat dimodelkan", e.baris)
            return st.env[e.nama][0]
        if isinstance(e, ast.Biner):
            return self._biner(e, st)
        if isinstance(e, ast.Unari):
            v = self.eval_bruto(e.operand, st)
            if e.op == "-":
                return -v
            if e.op == "!":
                return z3.Not(v)
            raise TidakDidukung("operator '~' belum dapat diverifikasi", e.baris)
        if isinstance(e, ast.Sbg):
            v = self.eval_bruto(e.nilai, st)
            lo, hi = hitung_rentang(e.tipe)
            if lo is not None:
                self.obligasi(st, z3.And(v >= lo, v <= hi),
                              f"konversi muat di {nama_tipe(e.tipe)}", e.baris)
            return v
        if isinstance(e, ast.Panggil):
            return self._panggil(e, st)
        if isinstance(e, ast.Medan):
            raise TidakDidukung("medan bentuk belum dapat diverifikasi", e.baris)
        if isinstance(e, ast.Indeks):
            raise TidakDidukung("akses deret belum dapat diverifikasi", e.baris)
        raise TidakDidukung(f"ekspresi {type(e).__name__} belum dapat diverifikasi", e.baris)

    def eval_bool(self, e, st):
        v = self.eval_bruto(e, st)
        if not z3.is_bool(v):
            raise TidakDidukung("ekspresi kontrak harus bertipe logika", e.baris)
        return v

    def obligasi(self, st, kondisi, keterangan, baris):
        if len(self.hasil.obligasi) >= BATAS_OBLIGASI:
            raise TidakDidukung("terlalu banyak obligasi; verifikasi dihentikan", baris)
        self.hasil.obligasi.append((list(st.path), list(st.fakta), kondisi, keterangan, baris))

    def _biner(self, e, st):
        op = e.op
        if op in ("dan", "&&", "atau", "||"):
            kiri = self.eval_bool(e.kiri, st)
            benar = op in ("dan", "&&")
            sub = Status(st.path + [kiri if benar else z3.Not(kiri)], st.fakta, st.env)
            kanan = self.eval_bool(e.kanan, sub)
            return z3.And(kiri, kanan) if benar else z3.Or(kiri, kanan)
        kiri = self.eval_bruto(e.kiri, st)
        kanan = self.eval_bruto(e.kanan, st)
        if op in ("+", "-", "*"):
            hasil = {"+": kiri + kanan, "-": kiri - kanan, "*": kiri * kanan}[op]
            lo, hi = hitung_rentang(e.tipe) if e.tipe is not None else (None, None)
            if lo is not None and not self.spec:
                self.obligasi(st, z3.And(hasil >= lo, hasil <= hi),
                              f"tanpa luapan '{op}' pada {nama_tipe(e.tipe)}", e.baris)
            return hasil
        if op in ("/", "%"):
            if e.tipe is not None and e.tipe.kind == "int":
                raise TidakDidukung(
                    "pembagian bilangan bulat belum dimodelkan eksak; "
                    "gunakan pecahan atau bantu dengan 'periksa'", e.baris)
            self.obligasi(st, kanan != 0, "pembagi tidak nol", e.baris)
            return kiri / kanan
        if op in ("==", "!=", "<", "<=", ">", ">="):
            return {"==": kiri == kanan, "!=": kiri != kanan, "<": kiri < kanan,
                    "<=": kiri <= kanan, ">": kiri > kanan, ">=": kiri >= kanan}[op]
        raise TidakDidukung(f"operator '{op}' belum dapat diverifikasi", e.baris)

    def _panggil(self, e, st):
        nama = getattr(e, "nama_fn", None)
        if nama == "cetak":
            return None
        if nama == "panjang":
            t = e.argumen[0].tipe
            if t.kind == "deret":
                return z3.IntVal(t.n)
            raise TidakDidukung("panjang teks belum dapat diverifikasi", e.baris)
        fn = self.k.fungsi.get(nama)
        if fn is None:
            raise TidakDidukung(f"fungsi '{nama}' tidak dikenal", e.baris)
        if not fn.murni or not (fn.syarat or fn.jamin):
            raise TidakDidukung(
                f"panggilan ke '{nama}' tidak dapat diverifikasi "
                "(tandai 'murni' dan beri syarat/jamin)", e.baris)
        if nama in self.tumpukan:
            raise TidakDidukung(f"panggilan rekursif ke '{nama}' belum didukung", e.baris)
        nilai_arg = [self.eval_bruto(a, st) for a in e.argumen]
        env = {p.nama: (v, tp) for p, v, tp in zip(fn.param, nilai_arg, fn.tipe_param)}
        sub = Status(path=st.path, fakta=st.fakta, env=env)
        for sy in fn.syarat:
            self.obligasi(st, self.eval_bool(sy, sub),
                          f"syarat '{nama}' dipenuhi pemanggil", e.baris)
        if fn.jamin and fn.tipe_hasil != KOSONG:
            hasil_fn = self._var(f"{nama}_hasil", fn.tipe_hasil, e)
            sub.env["hasil"] = (hasil_fn, fn.tipe_hasil)
            for jm in fn.jamin:
                st.fakta.append(self.eval_bool(jm, sub))
            return hasil_fn
        return z3.IntVal(0)

    # ---------- eksekusi ----------
    def jalankan(self, daftar, states):
        for s in daftar:
            states = self.langkah(s, states)
            if not states:
                break
        return states

    def jalankan_blok(self, blok, states):
        return self.jalankan(blok.isi, states)

    def langkah(self, s, states):
        if isinstance(s, ast.Dekl):
            for st in states:
                nilai = self.eval_bruto(s.nilai, st)
                st.env[s.nama] = (nilai, s.tipe_ter)
                lo, hi = hitung_rentang(s.tipe_ter)
                if lo is not None:
                    self.obligasi(st, z3.And(nilai >= lo, nilai <= hi),
                                  f"nilai awal '{s.nama}' muat", s.baris)
            return states
        if isinstance(s, ast.Rekah):
            return self._rekah(s, states)
        if isinstance(s, ast.Jika):
            keluar = []
            for st in states:
                cond = self.eval_bool(s.syarat, st)
                kiri = st.salinan()
                kiri.path.append(cond)
                keluar += self.jalankan_blok(s.maka, [kiri])
                kanan = st.salinan()
                kanan.path.append(z3.Not(cond))
                if isinstance(s.lainnya, ast.Blok):
                    keluar += self.jalankan_blok(s.lainnya, [kanan])
                elif isinstance(s.lainnya, ast.Jika):
                    keluar += self.langkah(s.lainnya, [kanan])
                else:
                    keluar.append(kanan)
            return keluar
        if isinstance(s, ast.Kembali):
            for st in states:
                nilai = self.eval_bruto(s.nilai, st) if s.nilai is not None else None
                self.hasil.keluar.append((list(st.path), list(st.fakta), nilai, s.baris))
            return []
        if isinstance(s, ast.Periksa):
            for st in states:
                self.obligasi(st, self.eval_bool(s.syarat, st),
                              s.pesan or "periksa", s.baris)
            return states
        if isinstance(s, ast.Cocok):
            return self._cocok(s, states)
        if isinstance(s, ast.Blok):
            return self.jalankan_blok(s, states)
        if isinstance(s, ast.Takaman):
            return self.jalankan_blok(s.badan, states)
        if isinstance(s, ast.EkspresiStmt):
            for st in states:
                if isinstance(s.ekspresi, ast.Panggil) and getattr(s.ekspresi, "nama_fn", None) == "cetak":
                    continue
                self.eval_bruto(s.ekspresi, st)
            return states
        if isinstance(s, ast.Selama):
            return self._selama(s, states)
        if isinstance(s, ast.Untuk):
            return self._untuk(s, states)
        if isinstance(s, ast.Ulang):
            raise TidakDidukung(
                "putaran 'ulang' tanpa syarat butuh invarian + varian; "
                "gunakan 'selama' atau 'untuk'", s.baris)
        if isinstance(s, (ast.Henti, ast.Lanjut)):
            raise TidakDidukung("henti/lanjut di luar konteks yang dimodelkan", s.baris)
        raise TidakDidukung(f"pernyataan {type(s).__name__} belum didukung", s.baris)

    # ---------- putaran dengan invarian ----------
    def _var_fresh(self, nama, tp):
        self._konter += 1
        return self._var(f"{nama}__{self._konter}", tp, ast.Simpul(baris=0))

    def _dimodifikasi(self, blok):
        hasil = set()

        def telusuri(b):
            for s in b.isi:
                if isinstance(s, ast.Rekah):
                    if isinstance(s.sasaran, ast.Nama):
                        hasil.add(s.sasaran.nama)
                elif isinstance(s, ast.Jika):
                    telusuri(s.maka)
                    if isinstance(s.lainnya, ast.Blok):
                        telusuri(s.lainnya)
                    elif isinstance(s.lainnya, ast.Jika):
                        telusuri(ast.Blok(baris=s.baris, isi=[s.lainnya]))
                elif isinstance(s, ast.Blok):
                    telusuri(s)
                elif isinstance(s, ast.Cocok):
                    for _, b2 in s.kasus:
                        telusuri(b2)
                    if s.lainnya is not None:
                        telusuri(s.lainnya)
                elif isinstance(s, ast.Takaman):
                    telusuri(s.badan)
        telusuri(blok)
        return hasil

    def _havoc(self, st, nama_nama):
        baru = st.salinan()
        for nama in sorted(nama_nama):
            if nama in baru.env:
                val, tp = baru.env[nama]
                baru.env[nama] = (self._var_fresh(nama, tp), tp)
        return baru

    def _invarian(self, s):
        if not s.invarian:
            raise TidakDidukung(
                "putaran butuh klausa 'invarian' agar dapat diverifikasi", s.baris)
        return s.invarian

    def _selama(self, s, states):
        self._invarian(s)
        keluar = []
        for st in states:
            self.spec += 1
            for iv in s.invarian:
                self.obligasi(st, self.eval_bool(iv, st),
                              "invarian berlaku saat masuk putaran", iv.baris)
            mod = self._dimodifikasi(s.badan)
            masuk = self._havoc(st, mod)
            inv_masuk = [self.eval_bool(iv, masuk) for iv in s.invarian]
            self.spec -= 1
            kondisi = self.eval_bool(s.syarat, masuk)
            masuk.path = list(st.path) + inv_masuk + [kondisi]
            akhir = self.jalankan_blok(s.badan, [masuk])
            self.spec += 1
            for stx in akhir:
                for iv in s.invarian:
                    self.obligasi(stx, self.eval_bool(iv, stx),
                                  "invarian terjaga setelah badan putaran", iv.baris)
            selesai = self._havoc(st, mod)
            inv_selesai = [self.eval_bool(iv, selesai) for iv in s.invarian]
            self.spec -= 1
            selesai.path = list(st.path) + inv_selesai + [z3.Not(self.eval_bool(s.syarat, selesai))]
            keluar.append(selesai)
        return keluar

    def _untuk(self, s, states):
        self._invarian(s)
        keluar = []
        for st0 in states:
            st = st0.salinan()
            st.env[s.nama] = (self.eval_bruto(s.awal, st), s.tipe_var)
            self.spec += 1
            for iv in s.invarian:
                self.obligasi(st, self.eval_bool(iv, st),
                              "invarian berlaku saat masuk putaran", iv.baris)
            mod = self._dimodifikasi(s.badan) | {s.nama}
            masuk = self._havoc(st, mod)
            inv_masuk = [self.eval_bool(iv, masuk) for iv in s.invarian]
            self.spec -= 1
            batas = self.eval_bruto(s.akhir, masuk)
            i_masuk = masuk.env[s.nama][0]
            kondisi = i_masuk <= batas if s.inklusif else i_masuk < batas
            masuk.path = list(st.path) + inv_masuk + [kondisi]
            akhir = self.jalankan_blok(s.badan, [masuk])
            for stx in akhir:
                i_akhir = stx.env[s.nama][0]
                lanjut = i_akhir + 1
                lo, hi = hitung_rentang(s.tipe_var)
                self.obligasi(stx, z3.And(lanjut >= lo, lanjut <= hi),
                              "penambahan variabel putaran tidak meluap", s.baris)
                stx.env[s.nama] = (lanjut, s.tipe_var)
                self.spec += 1
                for iv in s.invarian:
                    self.obligasi(stx, self.eval_bool(iv, stx),
                                  "invarian terjaga setelah badan putaran", iv.baris)
                self.spec -= 1
            selesai = self._havoc(st, mod)
            self.spec += 1
            inv_selesai = [self.eval_bool(iv, selesai) for iv in s.invarian]
            self.spec -= 1
            i_selesai = selesai.env[s.nama][0]
            batas2 = self.eval_bruto(s.akhir, selesai)
            kondisi2 = i_selesai <= batas2 if s.inklusif else i_selesai < batas2
            selesai.path = list(st.path) + inv_selesai + [z3.Not(kondisi2)]
            keluar.append(selesai)
        return keluar

    def _rekah(self, s, states):
        if not isinstance(s.sasaran, ast.Nama):
            raise TidakDidukung("penugasan ke elemen/medan belum dapat diverifikasi", s.baris)
        for st in states:
            if s.sasaran.nama not in st.env:
                raise TidakDidukung(
                    f"variabel '{s.sasaran.nama}' tidak dapat dimodelkan", s.baris)
            tipe = st.env[s.sasaran.nama][1]
            kanan = self.eval_bruto(s.nilai, st)
            if s.op == "=":
                baru = kanan
            else:
                kiri = st.env[s.sasaran.nama][0]
                op = s.op[0]
                if op in ("/", "%") and tipe.kind == "int":
                    raise TidakDidukung(
                        "pembagian bilangan bulat belum dimodelkan eksak", s.baris)
                if op in ("/", "%"):
                    self.obligasi(st, kanan != 0, "pembagi tidak nol", s.baris)
                baru = {"+": lambda: kiri + kanan, "-": lambda: kiri - kanan,
                        "*": lambda: kiri * kanan, "/": lambda: kiri / kanan,
                        "%": lambda: kiri % kanan}[op]()
                lo, hi = hitung_rentang(tipe)
                if lo is not None:
                    self.obligasi(st, z3.And(baru >= lo, baru <= hi),
                                  f"tanpa luapan '{s.op}' pada {nama_tipe(tipe)}", s.baris)
            st.env[s.sasaran.nama] = (baru, tipe)
        return states

    def _cocok(self, s, states):
        keluar = []
        for st in states:
            nilai = self.eval_bruto(s.nilai, st)
            sisa = st
            for pola, blok in s.kasus:
                if pola.jenis == "lainnya":
                    keluar += self.jalankan_blok(blok, [sisa])
                    sisa = None
                    break
                if s.tipe_nilai.kind == "pilihan":
                    cond = nilai == z3.IntVal(pola.indeks)
                else:
                    cond = nilai == z3.IntVal(pola.nilai)
                cabang = sisa.salinan()
                cabang.path.append(cond)
                keluar += self.jalankan_blok(blok, [cabang])
                sisa = sisa.salinan()
                sisa.path.append(z3.Not(cond))
            if sisa is not None and s.lainnya is not None:
                keluar += self.jalankan_blok(s.lainnya, [sisa])
        return keluar


def _ada_periksa(blok):
    for s in blok.isi:
        if isinstance(s, ast.Periksa):
            return True
        if isinstance(s, ast.Jika):
            if _ada_periksa(s.maka):
                return True
            if isinstance(s.lainnya, ast.Blok) and _ada_periksa(s.lainnya):
                return True
            if isinstance(s.lainnya, ast.Jika) and _ada_periksa(ast.Blok(isi=[s.lainnya])):
                return True
        if isinstance(s, ast.Blok) and _ada_periksa(s):
            return True
        if isinstance(s, ast.Takaman) and _ada_periksa(s.badan):
            return True
        if isinstance(s, ast.Cocok):
            if any(_ada_periksa(b) for _, b in s.kasus):
                return True
            if s.lainnya is not None and _ada_periksa(s.lainnya):
                return True
    return False


def _punya_kontrak(fn):
    return bool(fn.syarat or fn.jamin) or _ada_periksa(fn.badan)


def _tulis_smt(berkas, kondisi, param):
    deklarasi = set()

    def kumpulkan(e):
        if z3.is_const(e) and e.decl().kind() == z3.Z3_OP_UNINTERPRETED:
            deklarasi.add(e)
        if z3.is_app(e):
            for c in e.children():
                kumpulkan(c)

    kumpulkan(kondisi)
    ada_real = any(v.sort().eq(z3.RealSort()) for v in deklarasi)
    baris = [f"(set-logic {'ALL' if ada_real else 'QF_LIA'})"]
    for v in sorted(deklarasi, key=str):
        baris.append(f"(declare-fun {v} () {v.sort().sexpr()})")
    for nama, tp in param:
        lo, hi = hitung_rentang(tp)
        if lo is not None:
            baris.append(f"(assert (and (>= {nama} {lo}) (<= {nama} {hi})))")
    baris.append(f"(assert {kondisi.sexpr()})")
    baris.append("(check-sat)")
    with open(berkas, "w", encoding="utf-8") as f:
        f.write("\n".join(baris) + "\n")


def verifikasi_berkas(path, boleh_takaman=False, direktori_smt=None, verbose=False):
    with open(path, "r", encoding="utf-8") as f:
        sumber = f.read()
    program = parse(sumber, path)
    konteks = periksa(program, boleh_takaman=boleh_takaman)
    verif = Verifikator(konteks)
    target = [fn for fn in konteks.fungsi.values() if _punya_kontrak(fn)]
    if direktori_smt:
        os.makedirs(direktori_smt, exist_ok=True)
    if not target:
        print("tidak ada fungsi berkontrak (syarat/jamin) atau 'periksa' untuk diverifikasi")
        return 0
    terbukti = terbantah = tidak_pasti = 0
    print(f"bukti: {path}")
    for fn in target:
        hasil = verif.periksa_fn(fn)
        status_fn = "TERBUKTI"
        rincian = []
        param = [(f"{fn.nama}.{p.nama}".replace(".", "_"), tp)
                 for p, tp in zip(fn.param, fn.tipe_param)]
        for i, (pth, fakta, kondisi, ket, baris) in enumerate(hasil.obligasi):
            anda = list(pth) + list(fakta) + [z3.Not(kondisi)]
            for nama, tp in param:
                lo, hi = hitung_rentang(tp)
                if lo is not None:
                    anda.append(z3.Int(nama) >= lo)
                    anda.append(z3.Int(nama) <= hi)
            gabungan = z3.And(*anda) if anda else z3.BoolVal(True)
            solver = z3.Solver()
            solver.set("timeout", 10000)
            solver.add(gabungan)
            res = solver.check()
            if direktori_smt:
                berkas = os.path.join(direktori_smt, f"{fn.nama}_{i}.smt2")
                _tulis_smt(berkas, gabungan, param)
            if res == z3.unsat:
                rincian.append(("terbukti", baris, ket, None))
            elif res == z3.sat:
                status_fn = "TERBANTAH"
                model = solver.model()
                contoh = []
                for nama, tp in param:
                    sort = z3.IntSort() if tp.kind == "int" else z3.RealSort()
                    val = model.eval(z3.Const(nama, sort), model_completion=True)
                    contoh.append(f"{nama.split('.')[-1]} = {val}")
                rincian.append(("terbantah", baris, ket, ", ".join(contoh)))
            else:
                if status_fn == "TERBUKTI":
                    status_fn = "TIDAK PASTI"
                rincian.append(("tidak pasti", baris, ket, None))
        if hasil.unsupported and status_fn == "TERBUKTI":
            status_fn = "TIDAK PASTI"
        for baris, alasan in hasil.unsupported:
            rincian.append(("belum didukung", baris, alasan, None))
        if status_fn == "TERBUKTI":
            terbukti += 1
        elif status_fn == "TERBANTAH":
            terbantah += 1
        else:
            tidak_pasti += 1
        print(f"  {fn.nama}  [{status_fn}]  {len(hasil.obligasi)} obligasi, "
              f"{len(hasil.keluar)} jalur kembali")
        for jenis, baris, ket, contoh in rincian:
            if verbose or jenis in ("terbantah", "belum didukung", "tidak pasti"):
                extra = f" — contoh penyangkal: {contoh}" if contoh else ""
                print(f"      baris {baris}: {ket}: {jenis}{extra}")
    print(f"ringkasan: {terbukti} terbukti, {terbantah} terbantah, {tidak_pasti} tidak pasti")
    if terbantah:
        return 3
    if tidak_pasti:
        return 2
    return 0
