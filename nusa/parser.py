"""Parser rekursif-turun untuk bahasa Nusa."""
from . import ast
from .lexer import tokenkan

OPS_REKAH = ("=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=")


class GalatParse(Exception):
    def __init__(self, pesan, baris=0, kolom=0):
        super().__init__(f"baris {baris}:{kolom}: {pesan}")
        self.pesan = pesan
        self.baris = baris
        self.kolom = kolom


class Parser:
    def __init__(self, token):
        self.t = token
        self.i = 0

    # ---------- utilitas ----------
    @property
    def tok(self):
        return self.t[self.i]

    def lihat(self, k=0):
        j = self.i + k
        return self.t[j] if j < len(self.t) else self.t[-1]

    def cocok(self, jenis):
        if self.tok.jenis == jenis:
            self.i += 1
            return True
        return False

    def harap(self, jenis, petunjuk=""):
        if self.tok.jenis == jenis:
            self.i += 1
            return self.t[self.i - 1]
        tambahan = f" {petunjuk}" if petunjuk else ""
        raise GalatParse(
            f"mengharapkan {jenis!r}{tambahan}, tetapi menemukan {self.tok.jenis!r}",
            self.tok.baris, self.tok.kolom)

    def harap_id(self, petunjuk=""):
        if self.tok.jenis == "ID":
            self.i += 1
            return self.t[self.i - 1].nilai
        tambahan = f" {petunjuk}" if petunjuk else ""
        raise GalatParse(
            f"mengharapkan nama{tambahan}, tetapi menemukan {self.tok.jenis!r}",
            self.tok.baris, self.tok.kolom)

    # ---------- tingkat atas ----------
    def parse_program(self):
        butir = []
        while self.tok.jenis != "EOF":
            j = self.tok.jenis
            if j in ("murni", "fungsi"):
                butir.append(self.deklarasi_fungsi())
            elif j == "bentuk":
                butir.append(self.deklarasi_bentuk())
            elif j == "pilihan":
                butir.append(self.deklarasi_pilihan())
            elif j == "jenis":
                butir.append(self.deklarasi_jenis())
            else:
                raise GalatParse(
                    "deklarasi tingkat atas diharapkan: 'fungsi', 'bentuk', 'pilihan', atau 'jenis'",
                    self.tok.baris, self.tok.kolom)
        return ast.Program(baris=1, butir=butir)

    def deklarasi_fungsi(self):
        baris = self.tok.baris
        murni = self.cocok("murni")
        self.harap("fungsi")
        nama = self.harap_id("fungsi")
        self.harap("(")
        param = []
        if not self.cocok(")"):
            while True:
                pb = self.tok.baris
                pnama = self.harap_id("parameter")
                self.harap(":")
                pt = self.tipe()
                param.append(ast.Param(baris=pb, nama=pnama, tipe=pt))
                if self.cocok(","):
                    continue
                break
            self.harap(")")
        hasil = None
        if self.cocok("->"):
            hasil = self.tipe()
        syarat, jamin, invarian = [], [], []
        while self.tok.jenis in ("syarat", "jamin", "invarian"):
            j = self.tok.jenis
            self.i += 1
            e = self.ekspresi()
            {"syarat": syarat, "jamin": jamin, "invarian": invarian}[j].append(e)
        badan = self.blok()
        return ast.Fn(baris=baris, nama=nama, param=param, hasil=hasil,
                      syarat=syarat, jamin=jamin, invarian=invarian,
                      murni=murni, badan=badan)

    def deklarasi_bentuk(self):
        baris = self.tok.baris
        self.harap("bentuk")
        nama = self.harap_id("bentuk")
        self.harap("{")
        medan = []
        while not self.cocok("}"):
            mb = self.tok.baris
            mnama = self.harap_id("medan")
            self.harap(":")
            mt = self.tipe()
            medan.append((mnama, mt, mb))
            if self.cocok(",") or self.cocok(";"):
                continue
            self.harap("}", "penutup bentuk")
            break
        return ast.Bentuk(baris=baris, nama=nama, medan=medan)

    def deklarasi_pilihan(self):
        baris = self.tok.baris
        self.harap("pilihan")
        nama = self.harap_id("pilihan")
        self.harap("{")
        varian = []
        while not self.cocok("}") and self.tok.jenis != "EOF":
            varian.append(self.harap_id("varian"))
            if self.cocok(",") or self.cocok(";"):
                continue
            self.harap("}", "penutup pilihan")
            break
        return ast.Pilihan(baris=baris, nama=nama, varian=varian)

    def deklarasi_jenis(self):
        baris = self.tok.baris
        self.harap("jenis")
        nama = self.harap_id("jenis")
        self.harap("=")
        t = self.tipe()
        self.harap(";")
        return ast.Jenis(baris=baris, nama=nama, tipe=t)

    # ---------- tipe ----------
    def tipe(self):
        baris = self.tok.baris
        if self.tok.jenis != "ID":
            raise GalatParse(f"mengharapkan nama tipe, menemukan {self.tok.jenis!r}",
                             self.tok.baris, self.tok.kolom)
        nama = self.tok.nilai
        self.i += 1
        args = []
        if self.cocok("<"):
            while True:
                if self.tok.jenis == "INT":
                    args.append(ast.Angka(baris=self.tok.baris, nilai=self.tok.nilai))
                    self.i += 1
                else:
                    args.append(self.tipe())
                if self.cocok(","):
                    continue
                break
            self.harap(">")
        return ast.TipeRef(baris=baris, nama=nama, args=args)

    # ---------- pernyataan ----------
    def blok(self):
        baris = self.tok.baris
        self.harap("{")
        isi = []
        while self.tok.jenis != "}" and self.tok.jenis != "EOF":
            isi.append(self.pernyataan())
        self.harap("}", "penutup blok")
        return ast.Blok(baris=baris, isi=isi)

    def pernyataan(self):
        j = self.tok.jenis
        baris = self.tok.baris
        if j in ("tetap", "ubah"):
            self.i += 1
            nama = self.harap_id("variabel")
            tipe = None
            if self.cocok(":"):
                tipe = self.tipe()
            self.harap("=", "setiap variabel wajib diberi nilai awal")
            nilai = self.ekspresi()
            self.harap(";")
            return ast.Dekl(baris=baris, nama=nama, tipe=tipe, nilai=nilai, ubah=(j == "ubah"))
        if j == "jika":
            return self._jika()
        if j == "selama":
            self.i += 1
            syarat = self.ekspresi()
            invarian = []
            while self.cocok("invarian"):
                invarian.append(self.ekspresi())
            badan = self.blok()
            return ast.Selama(baris=baris, syarat=syarat, badan=badan, invarian=invarian)
        if j == "untuk":
            return self._untuk()
        if j == "ulang":
            self.i += 1
            return ast.Ulang(baris=baris, badan=self.blok())
        if j == "henti":
            self.i += 1
            self.harap(";")
            return ast.Henti(baris=baris)
        if j == "lanjut":
            self.i += 1
            self.harap(";")
            return ast.Lanjut(baris=baris)
        if j == "kembali":
            self.i += 1
            nilai = None
            if self.tok.jenis != ";":
                nilai = self.ekspresi()
            self.harap(";")
            return ast.Kembali(baris=baris, nilai=nilai)
        if j == "periksa":
            self.i += 1
            syarat = self.ekspresi()
            pesan = ""
            if self.cocok(","):
                pesan = self.harap("TEKS", "pesan periksa").nilai
            self.harap(";")
            return ast.Periksa(baris=baris, syarat=syarat, pesan=pesan)
        if j == "takaman":
            self.i += 1
            return ast.Takaman(baris=baris, badan=self.blok())
        if j == "cocok":
            return self._cocok()
        if j == "fungsi":
            raise GalatParse("fungsi tidak boleh dideklarasikan di dalam fungsi",
                             self.tok.baris, self.tok.kolom)
        if j == "{":
            return self.blok()
        eks = self.ekspresi()
        if self.tok.jenis in OPS_REKAH:
            op = self.tok.jenis
            self.i += 1
            nilai = self.ekspresi()
            self.harap(";")
            return ast.Rekah(baris=baris, sasaran=eks, op=op, nilai=nilai)
        self.harap(";", "akhir pernyataan")
        return ast.EkspresiStmt(baris=baris, ekspresi=eks)

    def _jika(self):
        baris = self.tok.baris
        self.harap("jika")
        syarat = self.ekspresi()
        maka = self.blok()
        lainnya = None
        if self.cocok("lainnya"):
            if self.tok.jenis == "jika":
                lainnya = self._jika()
            else:
                lainnya = self.blok()
        return ast.Jika(baris=baris, syarat=syarat, maka=maka, lainnya=lainnya)

    def _untuk(self):
        baris = self.tok.baris
        self.harap("untuk")
        nama = self.harap_id("variabel putaran")
        if self.cocok("dalam"):
            awal = self.ekspresi()
            if self.cocok("..="):
                inklusif = True
            elif self.cocok(".."):
                inklusif = False
            else:
                raise GalatParse("mengharapkan '..' atau '..=' pada rentang",
                                 self.tok.baris, self.tok.kolom)
            akhir = self.ekspresi()
        elif self.cocok("dari"):
            awal = self.ekspresi()
            self.harap("sampai")
            akhir = self.ekspresi()
            inklusif = True
        else:
            raise GalatParse("mengharapkan 'dalam' atau 'dari' setelah nama putaran",
                             self.tok.baris, self.tok.kolom)
        invarian = []
        while self.cocok("invarian"):
            invarian.append(self.ekspresi())
        badan = self.blok()
        return ast.Untuk(baris=baris, nama=nama, awal=awal, akhir=akhir,
                         inklusif=inklusif, badan=badan, invarian=invarian)

    def _cocok(self):
        baris = self.tok.baris
        self.harap("cocok")
        nilai = self.ekspresi()
        self.harap("{")
        kasus, lainnya = [], None
        while self.tok.jenis != "}":
            if self.cocok("kasus"):
                pb = self.tok.baris
                if self.tok.jenis == "ID" and self.tok.nilai == "_":
                    self.i += 1
                    pola = ast.Pola(baris=pb, jenis="lainnya")
                elif self.tok.jenis == "INT":
                    pola = ast.Pola(baris=pb, jenis="angka", nilai=self.tok.nilai)
                    self.i += 1
                elif self.tok.jenis in ("benar", "salah"):
                    pola = ast.Pola(baris=pb, jenis="angka",
                                    nilai=1 if self.tok.jenis == "benar" else 0)
                    self.i += 1
                elif self.tok.jenis == "ID":
                    nama = self.tok.nilai
                    self.i += 1
                    while self.cocok("."):
                        nama = self.harap_id("varian")
                    pola = ast.Pola(baris=pb, jenis="varian", nilai=nama)
                else:
                    raise GalatParse(f"pola tidak sah: {self.tok.jenis!r}",
                                     self.tok.baris, self.tok.kolom)
                kasus.append((pola, self.blok()))
            elif self.cocok("lainnya"):
                lainnya = self.blok()
            else:
                raise GalatParse("mengharapkan 'kasus' atau 'lainnya' di dalam 'cocok'",
                                 self.tok.baris, self.tok.kolom)
        self.harap("}")
        return ast.Cocok(baris=baris, nilai=nilai, kasus=kasus, lainnya=lainnya)

    # ---------- ekspresi ----------
    def ekspresi(self):
        return self._atau()

    def _biner_kiri(self, sub, daftar):
        kiri = sub()
        while self.tok.jenis in daftar:
            op = self.tok.jenis
            self.i += 1
            kanan = sub()
            kiri = ast.Biner(baris=kiri.baris, op=op, kiri=kiri, kanan=kanan)
        return kiri

    def _atau(self):
        return self._biner_kiri(self._dan, ("atau", "||"))

    def _dan(self):
        return self._biner_kiri(self._bit_atau, ("dan", "&&"))

    def _bit_atau(self):
        return self._biner_kiri(self._bit_xor, ("|",))

    def _bit_xor(self):
        return self._biner_kiri(self._bit_dan, ("^",))

    def _bit_dan(self):
        return self._biner_kiri(self._kesetaraan, ("&",))

    def _kesetaraan(self):
        return self._biner_kiri(self._relasi, ("==", "!="))

    def _relasi(self):
        return self._biner_kiri(self._geser, ("<", "<=", ">", ">="))

    def _geser(self):
        return self._biner_kiri(self._tambah, ("<<", ">>"))

    def _tambah(self):
        return self._biner_kiri(self._kali, ("+", "-"))

    def _kali(self):
        return self._biner_kiri(self._unari, ("*", "/", "%"))

    def _unari(self):
        if self.tok.jenis in ("-", "bukan", "!", "~"):
            baris = self.tok.baris
            op = "!" if self.tok.jenis in ("bukan", "!") else self.tok.jenis
            self.i += 1
            operand = self._unari()
            if op == "-" and isinstance(operand, (ast.Angka, ast.Pecahan)):
                # lipat '-' ke dalam literal agar batas negatif tipe bisa ditulis
                return type(operand)(baris=baris, nilai=-operand.nilai)
            return ast.Unari(baris=baris, op=op, operand=operand)
        e = self._pasca()
        while self.tok.jenis == "sebagai":
            self.i += 1
            e = ast.Sbg(baris=e.baris, nilai=e, tipe=self.tipe())
        return e

    def _pasca(self):
        e = self._primary()
        while True:
            if self.cocok("("):
                args = []
                if not self.cocok(")"):
                    while True:
                        args.append(self.ekspresi())
                        if self.cocok(","):
                            continue
                        break
                    self.harap(")")
                e = ast.Panggil(baris=e.baris, fungsi=e, argumen=args)
            elif self.cocok("["):
                idx = self.ekspresi()
                self.harap("]")
                e = ast.Indeks(baris=e.baris, dasar=e, indeks=idx)
            elif self.cocok("."):
                nama = self.harap_id("medan")
                e = ast.Medan(baris=e.baris, dasar=e, nama=nama)
            else:
                return e

    def _primary(self):
        t = self.tok
        if t.jenis == "INT":
            self.i += 1
            return ast.Angka(baris=t.baris, nilai=t.nilai)
        if t.jenis == "PECAHAN":
            self.i += 1
            return ast.Pecahan(baris=t.baris, nilai=t.nilai)
        if t.jenis == "TEKS":
            self.i += 1
            return ast.Teks(baris=t.baris, nilai=t.nilai)
        if t.jenis in ("benar", "salah"):
            self.i += 1
            return ast.Logika(baris=t.baris, nilai=(t.jenis == "benar"))
        if t.jenis == "(":
            self.i += 1
            e = self.ekspresi()
            self.harap(")")
            return e
        if t.jenis == "[":
            self.i += 1
            elemen = []
            if not self.cocok("]"):
                while True:
                    elemen.append(self.ekspresi())
                    if self.cocok(","):
                        continue
                    break
                self.harap("]")
            return ast.Deret(baris=t.baris, elemen=elemen)
        if t.jenis == "ID":
            self.i += 1
            if (self.tok.jenis == "{" and self.lihat(1).jenis == "ID"
                    and self.lihat(2).jenis == ":"):
                self.harap("{")
                medan = []
                while True:
                    mnama = self.harap_id("nama medan")
                    self.harap(":")
                    medan.append((mnama, self.ekspresi()))
                    if self.cocok(","):
                        continue
                    break
                self.harap("}")
                return ast.Struktur(baris=t.baris, nama=t.nilai, medan=medan)
            return ast.Nama(baris=t.baris, nama=t.nilai)
        raise GalatParse(f"ekspresi tidak sah: {t.jenis!r}", t.baris, t.kolom)


def parse(sumber, nama="<masukan>"):
    return Parser(tokenkan(sumber, nama)).parse_program()
