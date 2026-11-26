"""Penafsir (interpreter) bahasa Nusa.

Menjalankan AST secara langsung, tanpa LLVM dan tanpa penaut C. Semantik
sengaja dibuat identik dengan hasil kompilasi: nilai tetap/tidak tetap,
semantik nilai untuk `deret` dan `bentuk`, pemeriksaan batas indeks, luapan,
pembagian nol, geser, dan konversi. `uji/uji_nusa.py` membandingkan keluaran
penafsir dengan biner hasil kompilasi untuk memastikan keduanya sama.
"""
import math
import struct
import sys

from . import ast
from .tipe import KOSONG, hitung_rentang, nama_tipe


class Panik(Exception):
    """Kesalahan runtime yang pada biner memicu `abort()`."""

    def __init__(self, pesan, baris=0):
        super().__init__(pesan)
        self.pesan = pesan
        self.baris = baris


class GalatTafsir(Exception):
    pass


class KembaliSinyal(Exception):
    def __init__(self, nilai):
        self.nilai = nilai


class HentiSinyal(Exception):
    pass


class LanjutSinyal(Exception):
    pass


_KOSONG = object()


class Lingkungan:
    def __init__(self, induk=None):
        self.induk = induk
        self.isi = {}

    def pemilik(self, nama):
        e = self
        while e is not None:
            if nama in e.isi:
                return e
            e = e.induk
        return None

    def ambil(self, nama):
        nilai = self.isi.get(nama, _KOSONG)
        if nilai is not _KOSONG:
            return nilai
        e = self.induk
        while e is not None:
            nilai = e.isi.get(nama, _KOSONG)
            if nilai is not _KOSONG:
                return nilai
            e = e.induk
        raise GalatTafsir(f"variabel '{nama}' tidak dikenal saat tafsir")

    def simpan(self, nama, nilai):
        if nama in self.isi:
            self.isi[nama] = nilai
            return
        e = self.induk
        while e is not None:
            if nama in e.isi:
                e.isi[nama] = nilai
                return
            e = e.induk
        raise GalatTafsir(f"variabel '{nama}' tidak dikenal saat tafsir")

    def tetap(self, nama, nilai):
        self.isi[nama] = nilai


def _f32(x):
    try:
        return struct.unpack("<f", struct.pack("<f", x))[0]
    except OverflowError:
        return math.copysign(float("inf"), x)


class Penafsir:
    def __init__(self, konteks, keluaran=None, kedalaman_maks=2000):
        self.k = konteks
        self.fungsi = konteks.fungsi
        self.out = keluaran if keluaran is not None else sys.stdout
        self.lingkungan = Lingkungan()
        self.takaman = 0
        self.kedalaman = 0
        self.kedalaman_maks = kedalaman_maks

    # ---------- masuk ----------
    def jalankan(self):
        fn = self.fungsi["utama"]
        hasil = self._panggil_fn(fn, [], fn.baris)
        if fn.tipe_hasil.kind == KOSONG:
            return 0
        if isinstance(hasil, bool):
            return 1 if hasil else 0
        if isinstance(hasil, int):
            return hasil & 0xFF
        return 0

    # ---------- nilai bertipe ----------
    def _mask(self, tipe, v):
        v &= (1 << tipe.bits) - 1
        if tipe.signed and v >> (tipe.bits - 1):
            v -= 1 << tipe.bits
        return v

    def _cek_luap(self, tipe, v, op, baris):
        lo, hi = hitung_rentang(tipe)
        if v < lo or v > hi:
            raise Panik(f"luapan aritmetika {op} pada {nama_tipe(tipe)}", baris)
        return v

    def _bulat(self, op, a, b, tipe, baris):
        if op in ("/", "%"):
            if b == 0:
                raise Panik(f"pembagian dengan nol pada {nama_tipe(tipe)}"
                            if tipe.signed else "pembagian dengan nol", baris)
            if tipe.signed and a == -(1 << (tipe.bits - 1)) and b == -1:
                raise Panik("luapan pembagian minimum dengan -1", baris)
            if tipe.signed:
                q = abs(a) // abs(b)
                if (a < 0) != (b < 0):
                    q = -q
            else:
                q = a // b
            return q if op == "/" else a - q * b
        if op == "+":
            v = a + b
        elif op == "-":
            v = a - b
        else:
            v = a * b
        if self.takaman:
            return self._mask(tipe, v)
        return self._cek_luap(tipe, v, op, baris)

    def _pecahan(self, op, a, b, tipe):
        if op == "%":
            v = math.fmod(a, b)
        elif op == "/":
            v = self._bagi_pecahan(a, b)
        elif op == "+":
            v = a + b
        elif op == "-":
            v = a - b
        else:
            v = a * b
        return _f32(v) if tipe.bits == 32 else v

    @staticmethod
    def _bagi_pecahan(a, b):
        if b == 0.0:
            if a == 0.0 or a != a:
                return float("nan")
            return math.copysign(float("inf"), a) * math.copysign(1.0, b)
        return a / b

    def _hitung(self, op, a, b, tipe, baris):
        if tipe.kind == "float":
            return self._pecahan(op, a, b, tipe)
        return self._bulat(op, a, b, tipe, baris)

    def _salin(self, nilai, tipe):
        if nilai is None or tipe is None:
            return nilai
        if tipe.kind == "deret":
            return [self._salin(v, tipe.elem) for v in nilai]
        if tipe.kind == "bentuk":
            return [self._salin(v, ft) for v, (_, ft) in zip(nilai, tipe.medan)]
        return nilai

    # ---------- alamat ----------
    def _cek_indeks(self, tipe_dasar, i, baris):
        n = tipe_dasar.n
        if i < 0 or i >= n:
            raise Panik(f"indeks deret di luar batas 0..{n - 1}", baris)

    @staticmethod
    def _posisi_medan(tipe, nama):
        for i, (n, _) in enumerate(tipe.medan):
            if n == nama:
                return i
        raise GalatTafsir(f"medan '{nama}' tidak ada pada {tipe.nama}")

    def _wadah(self, e):
        if type(e) is ast.Nama:
            nilai = self.lingkungan.ambil(e.nama)
            if not isinstance(nilai, list):
                raise GalatTafsir(f"'{e.nama}' bukan deret atau bentuk")
            return nilai
        if type(e) is ast.Indeks:
            dasar = self._wadah(e.dasar)
            i = self._nilai(e.indeks)
            self._cek_indeks(e.dasar.tipe, i, e.baris)
            return dasar[i]
        if type(e) is ast.Medan:
            return self._wadah(e.dasar)[self._posisi_medan(e.dasar.tipe, e.nama)]
        raise GalatTafsir("sasaran bukan sesuatu yang bisa diindeks")

    def _baca(self, e):
        jenis = type(e)
        if jenis is ast.Nama:
            return self.lingkungan.ambil(e.nama)
        if jenis is ast.Indeks:
            wadah = self._wadah(e.dasar)
            i = self._nilai(e.indeks)
            self._cek_indeks(e.dasar.tipe, i, e.baris)
            return wadah[i]
        if jenis is ast.Medan:
            dasar = e.dasar
            if type(dasar) is ast.Nama and self.lingkungan.pemilik(dasar.nama) is None:
                return e.tipe.varian.index(e.nama)
            return self._wadah(dasar)[self._posisi_medan(dasar.tipe, e.nama)]
        return self._nilai(e)

    def _tulis(self, e, nilai, tipe):
        nilai = self._salin(nilai, tipe)
        if isinstance(e, ast.Nama):
            self.lingkungan.simpan(e.nama, nilai)
            return
        if isinstance(e, ast.Indeks):
            wadah = self._wadah(e.dasar)
            i = self._nilai(e.indeks)
            self._cek_indeks(e.dasar.tipe, i, e.baris)
            wadah[i] = nilai
            return
        if isinstance(e, ast.Medan):
            self._wadah(e.dasar)[self._posisi_medan(e.dasar.tipe, e.nama)] = nilai
            return
        raise GalatTafsir("sasaran penugasan tidak dikenal")

    # ---------- ekspresi ----------
    def _nilai(self, e):
        jenis = type(e)
        if jenis is ast.Angka:
            return float(e.nilai) if e.tipe.kind == "float" else e.nilai
        if jenis is ast.Nama:
            return self._baca(e)
        if jenis is ast.Biner:
            return self._biner(e)
        if jenis is ast.Panggil:
            return self._panggil(e)
        if jenis is ast.Indeks:
            return self._baca(e)
        if jenis is ast.Medan:
            return self._baca(e)
        if jenis is ast.Pecahan:
            return _f32(e.nilai) if e.tipe.bits == 32 else e.nilai
        if jenis is ast.Teks:
            return e.nilai
        if jenis is ast.Logika:
            return e.nilai
        if jenis is ast.Unari:
            return self._unari(e)
        if jenis is ast.Sbg:
            return self._sbg(e)
        if jenis is ast.Deret:
            return [self._salin(self._nilai(x), x.tipe) for x in e.elemen]
        if jenis is ast.Struktur:
            tipe = e.tipe
            nilai = [None] * len(tipe.medan)
            for nama, expr in e.medan:
                nilai[self._posisi_medan(tipe, nama)] = self._salin(self._nilai(expr), expr.tipe)
            return nilai
        raise GalatTafsir(f"ekspresi tidak dikenal: {jenis.__name__}")

    def _biner(self, e):
        op = e.op
        if op in ("dan", "&&"):
            return bool(self._nilai(e.kiri)) and bool(self._nilai(e.kanan))
        if op in ("atau", "||"):
            return bool(self._nilai(e.kiri)) or bool(self._nilai(e.kanan))
        a = self._nilai(e.kiri)
        b = self._nilai(e.kanan)
        tipe = e.tipe
        if op in ("+", "-", "*", "/", "%"):
            dasar = tipe if tipe.kind in ("int", "float") else e.kiri.tipe
            return self._hitung(op, a, b, dasar, e.baris)
        if op in ("==", "!=", "<", "<=", ">", ">="):
            tk = e.kiri.tipe
            if tk.kind == "teks":
                if op == "==":
                    return a == b
                if op == "!=":
                    return a != b
                cmp = (a > b) - (a < b)
                return {"<": cmp < 0, "<=": cmp <= 0, ">": cmp > 0, ">=": cmp >= 0}[op]
            if tk.kind == "pilihan":
                a = int(a)
            return {"==": a == b, "!=": a != b, "<": a < b, "<=": a <= b,
                    ">": a > b, ">=": a >= b}[op]
        if op in ("&", "|", "^"):
            tk = e.kiri.tipe
            hasil = {"&": a & b, "|": a | b, "^": a ^ b}[op]
            return self._mask(tk, hasil) if tk.kind == "int" else hasil
        if op in ("<<", ">>"):
            tk = e.kiri.tipe
            if not self.takaman:
                if b < 0 or b >= tk.bits:
                    raise Panik(f"geser melebihi {tk.bits} bit", e.baris)
            else:
                b &= 63
            hasil = (a << b) if op == "<<" else (a >> b)
            return self._mask(tk, hasil)
        raise GalatTafsir(f"operator '{op}' tidak dikenal pada penafsir")

    def _unari(self, e):
        v = self._nilai(e.operand)
        if e.op == "!":
            return not bool(v)
        if e.op == "-":
            tipe = e.tipe
            if tipe.kind == "float":
                return -v
            if self.takaman:
                return self._mask(tipe, -v)
            lo, _ = hitung_rentang(tipe)
            if v == lo:
                raise Panik(f"negasi meluap pada {nama_tipe(tipe)}", e.baris)
            return -v
        if e.op == "~":
            return self._mask(e.tipe, ~v)
        raise GalatTafsir(f"operator unari '{e.op}' tidak dikenal")

    def _sbg(self, e):
        asal = e.nilai.tipe
        tujuan = e.tipe
        v = self._nilai(e.nilai)
        if asal == tujuan:
            return v
        if asal.kind == "logika" and tujuan.kind == "int":
            return self._mask(tujuan, 1 if v else 0)
        if asal.kind == "int" and tujuan.kind == "int":
            return self._mask(tujuan, v)
        if asal.kind == "int" and tujuan.kind == "float":
            v = float(v)
            return _f32(v) if tujuan.bits == 32 else v
        if asal.kind == "float" and tujuan.kind == "float":
            return _f32(v) if tujuan.bits == 32 else v
        if asal.kind == "float" and tujuan.kind == "int":
            if not self.takaman:
                lo, hi = hitung_rentang(tujuan)
                if not (float(lo) <= v < (hi + 1)):
                    raise Panik(f"nilai pecahan di luar rentang {nama_tipe(tujuan)}", e.baris)
            if v != v or v in (float("inf"), float("-inf")):
                return 0
            return self._mask(tujuan, int(v))
        raise GalatTafsir(f"konversi {nama_tipe(asal)} ke {nama_tipe(tujuan)} belum didukung")

    # ---------- pemanggilan ----------
    def _panggil(self, e):
        nama = getattr(e, "nama_fn", None) or (e.fungsi.nama if isinstance(e.fungsi, ast.Nama) else "")
        if nama == "cetak":
            return self._cetak(e.argumen)
        if nama == "panjang":
            arg = e.argumen[0]
            nilai = self._nilai(arg)
            return len(nilai)
        fn = self.fungsi.get(nama)
        if fn is None:
            raise GalatTafsir(f"fungsi '{nama}' tidak dikenal saat tafsir")
        argumen = [self._nilai(a) for a in e.argumen]
        return self._panggil_fn(fn, argumen, e.baris)

    def _panggil_fn(self, fn, argumen, baris):
        self.kedalaman += 1
        if self.kedalaman > self.kedalaman_maks:
            self.kedalaman -= 1
            raise Panik("rekursi terlalu dalam untuk penafsir", baris)
        lama = self.lingkungan
        self.lingkungan = Lingkungan(lama)
        try:
            for p, tipe, nilai in zip(fn.param, fn.tipe_param, argumen):
                self.lingkungan.tetap(p.nama, self._salin(nilai, tipe))
            hasil = None
            try:
                self._blok(fn.badan, scope_baru=False)
            except KembaliSinyal as k:
                hasil = k.nilai
        finally:
            self.lingkungan = lama
            self.kedalaman -= 1
        if fn.tipe_hasil.kind == KOSONG:
            return None
        return self._salin(hasil, fn.tipe_hasil)

    def _cetak(self, argumen):
        bagian = []
        for arg in argumen:
            t = arg.tipe
            v = self._nilai(arg)
            if t.kind == "int":
                bagian.append(str(v))
            elif t.kind == "float":
                bagian.append("%g" % (v,))
            elif t.kind == "logika":
                bagian.append("benar" if v else "salah")
            elif t.kind == "teks":
                bagian.append(v)
            elif t.kind == "pilihan":
                bagian.append(t.varian[v] if 0 <= int(v) < len(t.varian) else "?")
            else:
                raise GalatTafsir(f"cetak tidak mendukung {nama_tipe(t)}")
        self.out.write(" ".join(bagian) + "\n")
        return None

    # ---------- pernyataan ----------
    def _blok(self, blok, scope_baru=True):
        if scope_baru:
            lama = self.lingkungan
            self.lingkungan = Lingkungan(lama)
        try:
            for s in blok.isi:
                self._pernyataan(s)
        finally:
            if scope_baru:
                self.lingkungan = lama

    def _pernyataan(self, s):
        jenis = type(s)
        if jenis is ast.Rekah:
            if s.op == "=":
                nilai = self._nilai(s.nilai)
            else:
                op = s.op[:-1] if s.op.endswith("=") else s.op
                nilai = self._hitung(op, self._baca(s.sasaran), self._nilai(s.nilai),
                                     s.sasaran.tipe, s.baris)
            self._tulis(s.sasaran, nilai, s.sasaran.tipe)
            return
        if jenis is ast.EkspresiStmt:
            self._nilai(s.ekspresi)
            return
        if jenis is ast.Jika:
            if self._nilai(s.syarat):
                self._blok(s.maka)
            elif s.lainnya is not None:
                if type(s.lainnya) is ast.Blok:
                    self._blok(s.lainnya)
                else:
                    self._pernyataan(s.lainnya)
            return
        if jenis is ast.Dekl:
            self.lingkungan.tetap(s.nama, self._salin(self._nilai(s.nilai), s.tipe_ter))
            return
        if isinstance(s, ast.Dekl):
            nilai = self._salin(self._nilai(s.nilai), s.tipe_ter)
            self.lingkungan.tetap(s.nama, nilai)
        elif isinstance(s, ast.Rekah):
            if s.op == "=":
                nilai = self._nilai(s.nilai)
            else:
                op = s.op[:-1] if s.op.endswith("=") else s.op
                tipe = s.sasaran.tipe
                nilai = self._hitung(op, self._baca(s.sasaran), self._nilai(s.nilai), tipe, s.baris)
            self._tulis(s.sasaran, nilai, s.sasaran.tipe)
        elif isinstance(s, ast.Jika):
            if self._nilai(s.syarat):
                self._blok(s.maka)
            elif s.lainnya is not None:
                if isinstance(s.lainnya, ast.Blok):
                    self._blok(s.lainnya)
                else:
                    self._pernyataan(s.lainnya)
        elif isinstance(s, ast.Selama):
            lama = self.lingkungan
            self.lingkungan = Lingkungan(lama)
            try:
                while self._nilai(s.syarat):
                    self.lingkungan.isi.clear()
                    try:
                        for pernyataan in s.badan.isi:
                            self._pernyataan(pernyataan)
                    except HentiSinyal:
                        break
                    except LanjutSinyal:
                        continue
            finally:
                self.lingkungan = lama
        elif isinstance(s, ast.Untuk):
            tipe = s.tipe_var
            i = self._nilai(s.awal)
            batas = self._nilai(s.akhir)
            lama = self.lingkungan
            self.lingkungan = Lingkungan(lama)
            try:
                self.lingkungan.tetap(s.nama, i)
                while (i <= batas) if s.inklusif else (i < batas):
                    try:
                        for pernyataan in s.badan.isi:
                            self._pernyataan(pernyataan)
                    except HentiSinyal:
                        break
                    except LanjutSinyal:
                        pass
                    i = self._bulat("+", i, 1, tipe, s.baris)
                    self.lingkungan.isi[s.nama] = i
                    for nama in list(self.lingkungan.isi):
                        if nama != s.nama:
                            del self.lingkungan.isi[nama]
            finally:
                self.lingkungan = lama
        elif isinstance(s, ast.Ulang):
            lama = self.lingkungan
            self.lingkungan = Lingkungan(lama)
            try:
                while True:
                    self.lingkungan.isi.clear()
                    try:
                        for pernyataan in s.badan.isi:
                            self._pernyataan(pernyataan)
                    except HentiSinyal:
                        break
                    except LanjutSinyal:
                        continue
            finally:
                self.lingkungan = lama
        elif isinstance(s, ast.Henti):
            raise HentiSinyal()
        elif isinstance(s, ast.Lanjut):
            raise LanjutSinyal()
        elif isinstance(s, ast.Kembali):
            nilai = None if s.nilai is None else self._salin(self._nilai(s.nilai), s.nilai.tipe)
            raise KembaliSinyal(nilai)
        elif isinstance(s, ast.Periksa):
            if not self._nilai(s.syarat):
                raise Panik(s.pesan or "periksa gagal", s.baris)
        elif isinstance(s, ast.Takaman):
            self.takaman += 1
            try:
                self._blok(s.badan)
            finally:
                self.takaman -= 1
        elif isinstance(s, ast.Cocok):
            nilai = self._nilai(s.nilai)
            for pola, blok in s.kasus:
                if self._cocok_pola(pola, nilai, s.nilai.tipe):
                    self._blok(blok)
                    return
            if s.lainnya is not None:
                self._blok(s.lainnya)
        elif isinstance(s, ast.Blok):
            self._blok(s)
        elif isinstance(s, ast.EkspresiStmt):
            self._nilai(s.ekspresi)
        else:
            raise GalatTafsir(f"pernyataan tidak dikenal: {type(s).__name__}")

    @staticmethod
    def _cocok_pola(pola, nilai, tipe):
        if pola.jenis == "lainnya":
            return True
        if pola.jenis == "varian":
            daftar = tipe.varian
            return daftar.index(pola.nilai) == int(nilai) if pola.nilai in daftar else False
        if pola.jenis == "angka":
            return pola.nilai == nilai
        return False
