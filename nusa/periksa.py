"""Pemeriksa tipe dan semantik bahasa Nusa."""
from dataclasses import dataclass

from . import ast
from .tipe import (DASAR, KOSONG, LOGIKA, PECAHAN, TEKS, Tipe,
                   apakah_angka, apakah_bulat, deret, hitung_rentang, nama_tipe)

BAWAAN_CETAK = ("int", "float", "logika", "teks", "pilihan")


class GalatSemantik(Exception):
    def __init__(self, daftar):
        self.daftar = daftar
        super().__init__("\n".join(f"baris {b}: {p}" for b, p in daftar))


@dataclass
class Variabel:
    tipe: Tipe
    ubah: bool = False


def _literal(e):
    """Nilai Python untuk literal bilangan, atau None."""
    if isinstance(e, ast.Angka):
        return e.nilai
    if isinstance(e, ast.Pecahan):
        return e.nilai
    if isinstance(e, ast.Unari) and e.op == "-" and isinstance(e.operand, (ast.Angka, ast.Pecahan)):
        return -e.operand.nilai
    return None


class Pemeriksa:
    def __init__(self, program, boleh_takaman=False):
        self.program = program
        self.boleh_takaman = boleh_takaman
        self.galat = []
        self.peringatan = []
        self.tref_bentuk = {}
        self.tref_pilihan = {}
        self.alias = {}
        self.tipe_bentuk = {}
        self.tipe_pilihan = {}
        self.fungsi = {}
        self.scope = []
        self.fn_hasil = KOSONG
        self.takaman = False
        self.loop = 0

    # ---------- utilitas galat ----------
    def g(self, simpul, pesan):
        self.galat.append((getattr(simpul, "baris", 0), pesan))

    # ---------- pendaftaran ----------
    def kumpulkan(self):
        for b in self.program.butir:
            if isinstance(b, ast.Bentuk):
                if b.nama in self.tref_bentuk or b.nama in self.tref_pilihan:
                    self.g(b, f"tipe '{b.nama}' sudah dideklarasikan")
                self.tref_bentuk[b.nama] = b
            elif isinstance(b, ast.Pilihan):
                if b.nama in self.tref_bentuk or b.nama in self.tref_pilihan:
                    self.g(b, f"tipe '{b.nama}' sudah dideklarasikan")
                self.tref_pilihan[b.nama] = b
            elif isinstance(b, ast.Jenis):
                if b.nama in self.alias or b.nama in self.tref_bentuk or b.nama in self.tref_pilihan:
                    self.g(b, f"tipe '{b.nama}' sudah dideklarasikan")
                self.alias[b.nama] = b
            elif isinstance(b, ast.Fn):
                if b.nama in self.fungsi:
                    self.g(b, f"fungsi '{b.nama}' sudah dideklarasikan")
                self.fungsi[b.nama] = b
            else:
                self.g(b, "butir tingkat atas tidak dikenal")

    def selesaikan(self, tref, tumpukan=()):
        if tref is None:
            return KOSONG
        nama = tref.nama
        if nama in self.alias:
            if nama in tumpukan:
                self.g(tref, f"jenis '{nama}' merujuk dirinya sendiri")
                return KOSONG
            return self.selesaikan(self.alias[nama].tipe, tumpukan + (nama,))
        if nama == "deret":
            if len(tref.args) != 2:
                self.g(tref, "deret memerlukan dua argumen: deret<tipe, panjang>")
                return KOSONG
            elem = self.selesaikan(tref.args[0], tumpukan)
            if not isinstance(tref.args[1], ast.Angka):
                self.g(tref, "panjang deret harus bilangan bulat literal")
                return KOSONG
            n = tref.args[1].nilai
            if n <= 0:
                self.g(tref, "panjang deret harus lebih besar dari nol")
                return KOSONG
            return deret(elem, n)
        if nama in DASAR:
            return DASAR[nama]
        if nama in self.tipe_bentuk:
            return self.tipe_bentuk[nama]
        if nama in self.tipe_pilihan:
            return self.tipe_pilihan[nama]
        self.g(tref, f"tipe tidak dikenal: '{nama}'")
        return KOSONG

    def siapkan_tipe(self):
        for nama, b in self.tref_pilihan.items():
            if len(set(b.varian)) != len(b.varian):
                self.g(b, f"pilihan '{nama}' memiliki varian ganda")
            self.tipe_pilihan[nama] = Tipe("pilihan", nama=nama, varian=tuple(b.varian))
        menunggu = dict(self.tref_bentuk)
        selesai = set()
        for _ in range(len(menunggu) + 1):
            for nama in list(menunggu):
                b = menunggu[nama]
                if any(nama == mt.nama for _, mt, _ in b.medan):
                    self.g(b, f"bentuk '{nama}' tidak boleh memuat dirinya sendiri")
                    del menunggu[nama]
                    self.tipe_bentuk[nama] = Tipe("bentuk", nama=nama, medan=())
                    selesai.add(nama)
                    continue
                if all(mt.nama in DASAR or mt.nama in self.alias or mt.nama == "deret"
                       or mt.nama in self.tipe_bentuk or mt.nama in self.tipe_pilihan
                       for _, mt, _ in b.medan):
                    medan = []
                    dilihat = set()
                    for mn, mt, mb in b.medan:
                        if mn in dilihat:
                            self.g(mb, f"medan '{mn}' ganda pada '{nama}'")
                        dilihat.add(mn)
                        medan.append((mn, self.selesaikan(mt)))
                    self.tipe_bentuk[nama] = Tipe("bentuk", nama=nama, medan=tuple(medan))
                    selesai.add(nama)
                    del menunggu[nama]
        for nama in list(menunggu):
            self.g(menunggu[nama], f"bentuk '{nama}' merujuk tipe yang tidak dikenal")

    # ---------- scope ----------
    def dorong(self):
        self.scope.append({})

    def tarik(self):
        self.scope.pop()

    def cari(self, nama):
        for s in reversed(self.scope):
            if nama in s:
                return s[nama]
        return None

    def deklarasikan(self, nama, var, simpul):
        if nama in self.scope[-1]:
            self.g(simpul, f"'{nama}' sudah dideklarasikan di blok ini")
            return
        self.scope[-1][nama] = var

    # ---------- masuk ----------
    def periksa(self):
        self.kumpulkan()
        self.siapkan_tipe()
        if "utama" not in self.fungsi:
            self.galat.append((0, "program harus memiliki fungsi 'utama'"))
        for fn in self.fungsi.values():
            self.periksa_fn(fn)
        if self.galat:
            raise GalatSemantik(sorted(self.galat))
        return self

    def periksa_fn(self, fn):
        fn.tipe_param = [self.selesaikan(p.tipe) for p in fn.param]
        fn.tipe_hasil = self.selesaikan(fn.hasil) if fn.hasil is not None else KOSONG
        if fn.nama == "utama":
            if fn.param:
                self.g(fn, "fungsi 'utama' tidak boleh memiliki parameter")
            if fn.tipe_hasil not in (KOSONG, DASAR["bulat"], DASAR["bulat32"]):
                self.g(fn, "fungsi 'utama' harus mengembalikan kosong atau bulat")
        nama_param = set()
        for p in fn.param:
            if p.nama in nama_param:
                self.g(p, f"parameter '{p.nama}' ganda")
            nama_param.add(p.nama)
        self.fn_hasil = fn.tipe_hasil
        self.takaman = False
        self.loop = 0
        self.dorong()
        for p, tp in zip(fn.param, fn.tipe_param):
            self.deklarasikan(p.nama, Variabel(tp, False), p)
        for e in fn.syarat:
            self.periksa_ekspresi(e, LOGIKA, "syarat")
        for e in fn.jamin:
            self.dorong()
            self.deklarasikan("hasil", Variabel(fn.tipe_hasil, False), fn)
            self.periksa_ekspresi(e, LOGIKA, "jamin")
            self.tarik()
        for e in fn.invarian:
            self.periksa_ekspresi(e, LOGIKA, "invarian")
        self.periksa_blok(fn.badan, scope_baru=False)
        fn.pasti_kembali = self.blok_pasti_kembali(fn.badan)
        if fn.tipe_hasil != KOSONG and not fn.pasti_kembali:
            self.g(fn, f"fungsi '{fn.nama}' harus mengembalikan nilai di semua jalur")
        self.tarik()

    # ---------- pernyataan ----------
    def periksa_blok(self, blok, scope_baru=True):
        if scope_baru:
            self.dorong()
        for s in blok.isi:
            self.periksa_pernyataan(s)
        if scope_baru:
            self.tarik()

    def periksa_pernyataan(self, s):
        if isinstance(s, ast.Dekl):
            self._dekl(s)
        elif isinstance(s, ast.Rekah):
            self._rekah(s)
        elif isinstance(s, ast.Jika):
            self.periksa_ekspresi(s.syarat, LOGIKA, "syarat jika")
            self.periksa_blok(s.maka)
            if isinstance(s.lainnya, ast.Blok):
                self.periksa_blok(s.lainnya)
            elif isinstance(s.lainnya, ast.Jika):
                self.periksa_pernyataan(s.lainnya)
        elif isinstance(s, ast.Selama):
            self.periksa_ekspresi(s.syarat, LOGIKA, "syarat selama")
            for e in s.invarian:
                self.periksa_ekspresi(e, LOGIKA, "invarian putaran")
            self.loop += 1
            self.periksa_blok(s.badan)
            self.loop -= 1
        elif isinstance(s, ast.Untuk):
            self._untuk(s)
        elif isinstance(s, ast.Ulang):
            self.loop += 1
            self.periksa_blok(s.badan)
            self.loop -= 1
        elif isinstance(s, (ast.Henti, ast.Lanjut)):
            if self.loop == 0:
                self.g(s, f"'{'henti' if isinstance(s, ast.Henti) else 'lanjut'}' hanya boleh di dalam putaran")
        elif isinstance(s, ast.Kembali):
            if self.fn_hasil == KOSONG:
                if s.nilai is not None:
                    self.g(s, "fungsi ini tidak mengembalikan nilai, hapus ekspresi setelah 'kembali'")
            else:
                if s.nilai is None:
                    self.g(s, f"'kembali' harus disertai nilai bertipe {nama_tipe(self.fn_hasil)}")
                else:
                    self.periksa_ekspresi(s.nilai, self.fn_hasil, "nilai kembali")
        elif isinstance(s, ast.Periksa):
            self.periksa_ekspresi(s.syarat, LOGIKA, "periksa")
        elif isinstance(s, ast.Takaman):
            if not self.boleh_takaman:
                self.g(s, "blok 'takaman' memerlukan opsi --boleh-takaman; "
                          "pemeriksaan runtime dimatikan di dalamnya")
                return
            self.peringatan.append((s.baris, "blok takaman: pemeriksaan runtime dimatikan"))
            lama = self.takaman
            self.takaman = True
            self.periksa_blok(s.badan)
            self.takaman = lama
        elif isinstance(s, ast.Cocok):
            self._cocok(s)
        elif isinstance(s, ast.EkspresiStmt):
            self.periksa_ekspresi(s.ekspresi)
        elif isinstance(s, ast.Blok):
            self.periksa_blok(s)
        else:
            self.g(s, f"pernyataan tidak dikenal: {type(s).__name__}")

    def _dekl(self, s):
        if s.tipe is not None:
            tipe = self.selesaikan(s.tipe)
            if tipe == KOSONG:
                self.g(s, f"variabel '{s.nama}' tidak boleh bertipe kosong")
            self.periksa_ekspresi(s.nilai, tipe, f"nilai awal '{s.nama}'")
        else:
            tipe = self.periksa_ekspresi(s.nilai)
            if isinstance(s.nilai, ast.Angka):
                tipe = DASAR["bulat"]
                s.nilai.tipe = tipe
            if tipe == KOSONG:
                self.g(s, f"variabel '{s.nama}' tidak boleh bertipe kosong")
        s.tipe_ter = tipe
        self.deklarasikan(s.nama, Variabel(tipe, s.ubah), s)

    def _untuk(self, s):
        awal = self.periksa_ekspresi(s.awal)
        if not apakah_bulat(awal):
            self.g(s.awal, f"batas putaran harus bilangan bulat, bukan {nama_tipe(awal)}")
            awal = DASAR["bulat"]
        self.periksa_ekspresi(s.akhir, awal, "batas akhir putaran")
        s.tipe_var = awal
        self.dorong()
        self.deklarasikan(s.nama, Variabel(awal, False), s)
        for e in s.invarian:
            self.periksa_ekspresi(e, LOGIKA, "invarian putaran")
        self.loop += 1
        self.periksa_blok(s.badan, scope_baru=False)
        self.loop -= 1
        self.tarik()

    def _cocok(self, s):
        tn = self.periksa_ekspresi(s.nilai)
        s.tipe_nilai = tn
        ada_lainnya = s.lainnya is not None
        dipakai = set()
        for pola, _blok in s.kasus:
            if pola.jenis == "lainnya":
                if ada_lainnya:
                    self.g(pola, "'cocok' hanya boleh punya satu cabang cadangan")
                ada_lainnya = True
            elif tn.kind == "pilihan":
                if pola.jenis != "varian":
                    self.g(pola, f"pola bilangan tidak cocok untuk pilihan {tn.nama}")
                    continue
                if pola.nilai not in tn.varian:
                    self.g(pola, f"'{pola.nilai}' bukan varian dari pilihan {tn.nama}")
                    continue
                if pola.nilai in dipakai:
                    self.g(pola, f"varian '{pola.nilai}' sudah ditangani")
                dipakai.add(pola.nilai)
                pola.indeks = tn.varian.index(pola.nilai)
            elif tn.kind == "int":
                if pola.jenis != "angka":
                    self.g(pola, f"'{pola.nilai}' bukan bilangan; pilihan {nama_tipe(tn)} butuh pola angka")
                elif pola.nilai in dipakai:
                    self.g(pola, f"pola {pola.nilai} ganda")
                else:
                    dipakai.add(pola.nilai)
            elif tn.kind == "logika":
                if pola.jenis != "angka" or pola.nilai not in (0, 1):
                    self.g(pola, "pola untuk logika harus 'benar' atau 'salah'")
            else:
                self.g(s, f"'cocok' tidak mendukung tipe {nama_tipe(tn)}")
        s.lengkap = ada_lainnya
        if tn.kind == "pilihan" and not ada_lainnya:
            kurang = [v for v in tn.varian if v not in dipakai]
            if kurang:
                self.g(s, "cocok tidak lengkap; varian belum ditangani: " + ", ".join(kurang))
            else:
                s.lengkap = True
        for pola, blok in s.kasus:
            self.periksa_blok(blok)
        if s.lainnya is not None:
            self.periksa_blok(s.lainnya)

    def _rekah(self, s):
        tipe, dapat_diubah = self.lvalue(s.sasaran)
        if not dapat_diubah:
            self.g(s, "tidak dapat menugaskan ke nilai tetap (gunakan 'ubah' atau tanda 'takaman')")
        if s.op != "=":
            if not apakah_angka(tipe):
                self.g(s, f"operator '{s.op}' hanya untuk bilangan")
            self.periksa_ekspresi(s.nilai, tipe, "nilai penugasan")
        else:
            self.periksa_ekspresi(s.nilai, tipe, "nilai penugasan")

    def lvalue(self, e):
        if isinstance(e, ast.Nama):
            v = self.cari(e.nama)
            if v is None:
                self.g(e, f"variabel '{e.nama}' tidak dikenal")
                return KOSONG, False
            e.tipe = v.tipe
            return v.tipe, v.ubah
        if isinstance(e, ast.Indeks):
            tbase, ubah = self.lvalue(e.dasar)
            if tbase.kind != "deret":
                self.g(e, f"hanya deret yang dapat diindeks, bukan {nama_tipe(tbase)}")
                return KOSONG, False
            self.periksa_ekspresi(e.indeks, DASAR["bulat"], "indeks")
            e.tipe = tbase.elem
            return tbase.elem, ubah
        if isinstance(e, ast.Medan):
            tbase, ubah = self.lvalue(e.dasar)
            if tbase.kind != "bentuk":
                self.g(e, f"hanya bentuk yang memiliki medan, bukan {nama_tipe(tbase)}")
                return KOSONG, False
            for mn, mt in tbase.medan:
                if mn == e.nama:
                    e.tipe = mt
                    return mt, ubah
            self.g(e, f"bentuk {tbase.nama} tidak memiliki medan '{e.nama}'")
            return KOSONG, False
        self.g(e, "sisi kiri penugasan harus variabel, elemen deret, atau medan")
        return KOSONG, False

    # ---------- ekspresi ----------
    def periksa_ekspresi(self, e, harap=None, konteks=""):
        t = self._ekspresi(e, harap)
        e.tipe = t
        if harap is not None and t != KOSONG and t != harap:
            self.g(e, f"tipe tidak cocok{(' pada ' + konteks) if konteks else ''}: "
                      f"mengharapkan {nama_tipe(harap)}, mendapat {nama_tipe(t)}")
            return harap
        return t

    def _ekspresi(self, e, harap=None):
        if isinstance(e, ast.Angka):
            if harap is not None and harap.kind == "float":
                e.tipe = harap
                e.nilai_pecahan = float(e.nilai)
                return harap
            tipe = harap if (harap is not None and harap.kind == "int") else DASAR["bulat"]
            lo, hi = hitung_rentang(tipe)
            if not (lo <= e.nilai <= hi):
                self.g(e, f"literal {e.nilai} di luar rentang {nama_tipe(tipe)} ({lo}..{hi})")
            e.tipe = tipe
            return tipe
        if isinstance(e, ast.Pecahan):
            tipe = harap if (harap is not None and harap.kind == "float") else PECAHAN
            e.tipe = tipe
            return tipe
        if isinstance(e, ast.Teks):
            e.tipe = TEKS
            return TEKS
        if isinstance(e, ast.Logika):
            e.tipe = LOGIKA
            return LOGIKA
        if isinstance(e, ast.Nama):
            v = self.cari(e.nama)
            if v is None:
                if e.nama in self.fungsi:
                    self.g(e, f"fungsi '{e.nama}' harus dipanggil, bukan dipakai sebagai nilai")
                else:
                    self.g(e, f"nama '{e.nama}' tidak dikenal")
                return KOSONG
            e.tipe = v.tipe
            return v.tipe
        if isinstance(e, ast.Biner):
            return self._biner(e, harap)
        if isinstance(e, ast.Unari):
            return self._unari(e, harap)
        if isinstance(e, ast.Sbg):
            asal = self.periksa_ekspresi(e.nilai)
            tujuan = self.selesaikan(e.tipe)
            if asal == KOSONG:
                return tujuan
            ok = ((apakah_angka(asal) or asal.kind == "logika") and apakah_angka(tujuan))
            if not ok:
                self.g(e, f"konversi tidak diizinkan: {nama_tipe(asal)} sebagai {nama_tipe(tujuan)}")
            return tujuan
        if isinstance(e, ast.Deret):
            if not e.elemen:
                self.g(e, "deret literal tidak boleh kosong; tulis elemennya")
                return KOSONG
            elem_h = harap.elem if (harap is not None and harap.kind == "deret") else None
            tipe_elem = self.periksa_ekspresi(e.elemen[0], elem_h)
            if isinstance(e.elemen[0], ast.Angka) and elem_h is None:
                tipe_elem = DASAR["bulat"]
                e.elemen[0].tipe = tipe_elem
            for x in e.elemen[1:]:
                self.periksa_ekspresi(x, tipe_elem, "elemen deret")
            t = deret(tipe_elem, len(e.elemen))
            if harap is not None and harap.kind == "deret" and harap.n != t.n:
                self.g(e, f"panjang deret salah: mengharapkan {harap.n}, mendapat {t.n}")
            return t
        if isinstance(e, ast.Indeks):
            tbase = self.periksa_ekspresi(e.dasar)
            if tbase.kind != "deret":
                self.g(e, f"hanya deret yang dapat diindeks, bukan {nama_tipe(tbase)}")
                return KOSONG
            self.periksa_ekspresi(e.indeks, DASAR["bulat"], "indeks")
            return tbase.elem
        if isinstance(e, ast.Medan):
            return self._medan(e)
        if isinstance(e, ast.Struktur):
            return self._struktur(e, harap)
        if isinstance(e, ast.Panggil):
            return self._panggil(e)
        self.g(e, f"ekspresi tidak dikenal: {type(e).__name__}")
        return KOSONG

    def _biner(self, e, harap):
        op = e.op
        logika = op in ("dan", "atau", "&&", "||")
        banding = op in ("==", "!=", "<", "<=", ">", ">=")
        geser = op in ("<<", ">>")
        if logika:
            self.periksa_ekspresi(e.kiri, LOGIKA, f"kiri '{op}'")
            self.periksa_ekspresi(e.kanan, LOGIKA, f"kanan '{op}'")
            return LOGIKA
        lit_k, lit_n = _literal(e.kiri), _literal(e.kanan)
        if lit_k is not None and lit_n is not None:
            if isinstance(lit_k, float) or isinstance(lit_n, float):
                tk = self.periksa_ekspresi(e.kiri, PECAHAN)
                tn = self.periksa_ekspresi(e.kanan, tk)
            else:
                tipe = harap if (harap is not None and harap.kind in ("int", "float")) else DASAR["bulat"]
                tk = self.periksa_ekspresi(e.kiri, tipe)
                tn = self.periksa_ekspresi(e.kanan, tk)
        elif lit_k is not None:
            tn = self.periksa_ekspresi(e.kanan)
            tk = self.periksa_ekspresi(e.kiri, tn if apakah_angka(tn) else None)
        elif lit_n is not None:
            tk = self.periksa_ekspresi(e.kiri)
            tn = self.periksa_ekspresi(e.kanan, tk if apakah_angka(tk) else None)
        else:
            tk = self.periksa_ekspresi(e.kiri)
            tn = self.periksa_ekspresi(e.kanan)
        if geser:
            if not apakah_bulat(tk) or not apakah_bulat(tn):
                self.g(e, f"operator '{op}' hanya untuk bilangan bulat")
                return KOSONG
            return tk
        if banding:
            if tk != tn:
                self.g(e, f"perbandingan '{op}' antar tipe berbeda: {nama_tipe(tk)} dan {nama_tipe(tn)}")
            elif not (apakah_angka(tk) or tk.kind in ("logika", "teks", "pilihan")):
                self.g(e, f"tipe {nama_tipe(tk)} tidak dapat dibandingkan")
            elif tk.kind == "pilihan" and op not in ("==", "!="):
                self.g(e, "pilihan hanya dapat diuji dengan '==' atau '!='")
            return LOGIKA
        if op in ("&", "|", "^"):
            if not apakah_bulat(tk) or not apakah_bulat(tn):
                self.g(e, f"operator '{op}' hanya untuk bilangan bulat")
                return KOSONG
            return tk
        if not apakah_angka(tk):
            self.g(e, f"operator '{op}' hanya untuk bilangan, bukan {nama_tipe(tk)}")
            return KOSONG
        if tk != tn:
            self.g(e, f"operasi '{op}' antar tipe berbeda: {nama_tipe(tk)} dan {nama_tipe(tn)}")
        return tk

    def _unari(self, e, harap):
        if e.op == "!":
            self.periksa_ekspresi(e.operand, LOGIKA, "operand '!'")
            return LOGIKA
        t = self.periksa_ekspresi(e.operand, harap)
        if e.op == "-":
            if not apakah_angka(t):
                self.g(e, f"negasi hanya untuk bilangan, bukan {nama_tipe(t)}")
            elif t.kind == "int" and not t.signed:
                self.g(e, f"negasi tidak diizinkan pada {nama_tipe(t)}; gunakan konversi eksplisit")
        elif e.op == "~":
            if not apakah_bulat(t):
                self.g(e, f"'~' hanya untuk bilangan bulat, bukan {nama_tipe(t)}")
        return t

    def _medan(self, e):
        if isinstance(e.dasar, ast.Nama) and self.cari(e.dasar.nama) is None:
            tp = self.tipe_pilihan.get(e.dasar.nama)
            if tp is not None:
                if e.nama not in tp.varian:
                    self.g(e, f"pilihan {tp.nama} tidak memiliki varian '{e.nama}'")
                    return KOSONG
                e.tipe = tp
                e.indeks_varian = tp.varian.index(e.nama)
                e.konstanta_pilihan = True
                return tp
        tbase = self.periksa_ekspresi(e.dasar)
        if tbase.kind != "bentuk":
            self.g(e, f"hanya bentuk yang memiliki medan, bukan {nama_tipe(tbase)}")
            return KOSONG
        for mn, mt in tbase.medan:
            if mn == e.nama:
                return mt
        self.g(e, f"bentuk {tbase.nama} tidak memiliki medan '{e.nama}'")
        return KOSONG

    def _struktur(self, e, harap):
        if e.nama not in self.tipe_bentuk:
            self.g(e, f"bentuk '{e.nama}' tidak dikenal")
            return KOSONG
        tipe = self.tipe_bentuk[e.nama]
        nama_medan = [mn for mn, _ in tipe.medan]
        dilihat = {}
        for mn, eks in e.medan:
            if mn not in nama_medan:
                self.g(e, f"bentuk {tipe.nama} tidak memiliki medan '{mn}'")
                continue
            if mn in dilihat:
                self.g(e, f"medan '{mn}' diisi dua kali")
            dilihat[mn] = eks
        for mn, mt in tipe.medan:
            if mn not in dilihat:
                self.g(e, f"medan '{mn}' pada {tipe.nama} belum diisi")
            else:
                self.periksa_ekspresi(dilihat[mn], mt, f"medan '{mn}'")
        e.tipe = tipe
        e.medan_ter = dilihat
        return tipe

    def _panggil(self, e):
        if not isinstance(e.fungsi, ast.Nama):
            self.g(e, "hanya nama fungsi yang dapat dipanggil")
            return KOSONG
        nama = e.fungsi.nama
        if self.cari(nama) is not None:
            self.g(e, f"'{nama}' adalah variabel, bukan fungsi")
            return KOSONG
        if nama in self.fungsi:
            fn = self.fungsi[nama]
            if not hasattr(fn, "tipe_param"):
                # panggilan timbal balik sebelum fungsi diperiksa: selesaikan sekarang
                fn.tipe_param = [self.selesaikan(p.tipe) for p in fn.param]
                fn.tipe_hasil = self.selesaikan(fn.hasil) if fn.hasil is not None else KOSONG
            if len(e.argumen) != len(fn.tipe_param):
                self.g(e, f"fungsi '{nama}' memerlukan {len(fn.tipe_param)} argumen, "
                          f"diberikan {len(e.argumen)}")
            for i, (arg, tp) in enumerate(zip(e.argumen, fn.tipe_param)):
                self.periksa_ekspresi(arg, tp, f"argumen {i + 1} '{nama}'")
            e.nama_fn = nama
            return fn.tipe_hasil
        if nama == "cetak":
            for arg in e.argumen:
                t = self.periksa_ekspresi(arg)
                if t.kind not in BAWAAN_CETAK:
                    self.g(arg, f"'cetak' tidak dapat mencetak tipe {nama_tipe(t)}")
            e.nama_fn = "cetak"
            return KOSONG
        if nama == "panjang":
            if len(e.argumen) != 1:
                self.g(e, "'panjang' memerlukan satu argumen")
                return DASAR["bulat"]
            t = self.periksa_ekspresi(e.argumen[0])
            if t.kind not in ("deret", "teks"):
                self.g(e, f"'panjang' hanya untuk deret atau teks, bukan {nama_tipe(t)}")
            e.nama_fn = "panjang"
            return DASAR["bulat"]
        self.g(e, f"fungsi '{nama}' tidak dikenal")
        return KOSONG

    # ---------- analisis alur ----------
    def blok_pasti_kembali(self, blok):
        for s in blok.isi:
            if isinstance(s, ast.Kembali):
                return True
            if isinstance(s, ast.Jika) and s.lainnya is not None:
                if self.blok_pasti_kembali(s.maka) and self._cabang_kembali(s.lainnya):
                    return True
            if isinstance(s, ast.Cocok) and (s.lainnya is not None or getattr(s, "lengkap", False)):
                semua = all(self.blok_pasti_kembali(b) for _, b in s.kasus)
                if s.lainnya is None:
                    if semua:
                        return True
                elif semua and self.blok_pasti_kembali(s.lainnya):
                    return True
            if isinstance(s, ast.Takaman) and self.blok_pasti_kembali(s.badan):
                return True
        return False

    def _cabang_kembali(self, s):
        if isinstance(s, ast.Blok):
            return self.blok_pasti_kembali(s)
        if isinstance(s, ast.Jika):
            return (s.lainnya is not None
                    and self.blok_pasti_kembali(s.maka)
                    and self._cabang_kembali(s.lainnya))
        return self.blok_pasti_kembali(s.badan)


def periksa(program, boleh_takaman=False):
    return Pemeriksa(program, boleh_takaman).periksa()
