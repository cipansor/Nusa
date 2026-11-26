"""Lexer bahasa Nusa (.ns)."""
from dataclasses import dataclass

# Kata kunci kanonik -> singkatan dan padanan Latin yang diterima.
KATA_KUNCI = {
    "fungsi": ["fn"],
    "ubah": ["ub"],
    "tetap": ["tp"],
    "kembali": ["kmb", "return"],
    "jika": ["jk", "if"],
    "lainnya": ["ln", "else"],
    "selama": ["slm", "while"],
    "untuk": ["utk", "for"],
    "dalam": ["dlm", "in"],
    "dari": ["dr"],
    "sampai": ["sp"],
    "ulang": ["ul", "loop"],
    "henti": ["ht", "break"],
    "lanjut": ["lnj", "continue"],
    "benar": ["bnr", "true"],
    "salah": ["slh", "false"],
    "dan": ["and"],
    "atau": ["or"],
    "bukan": ["not"],
    "bentuk": ["bt", "struct"],
    "pilihan": ["plh", "enum"],
    "cocok": ["cck", "match"],
    "kasus": ["ks", "case"],
    "takaman": ["ta", "unsafe"],
    "syarat": ["syr", "requires"],
    "jamin": ["jmn", "ensures"],
    "invarian": ["inv", "invariant"],
    "periksa": ["prk", "assert"],
    "sebagai": ["sbg", "as"],
    "jenis": ["jns", "type"],
    "murni": ["mrn", "pure"],
}

# Nama bawaan yang tetap berupa identifier agar bisa dipanggil seperti fungsi.
SINONIM_ID = {
    "ctk": "cetak", "print": "cetak", "tulis": "cetak",
    "pjg": "panjang", "len": "panjang",
}
_ALIAS = set(KATA_KUNCI)
for _kanonik, _singkat in KATA_KUNCI.items():
    for _s in _singkat:
        _ALIAS.add(_s)

# Simbol, terpanjang lebih dulu agar pencocokan tidak salah.
SIMBOL = [
    "..=", "->", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>",
    "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "..",
    "+", "-", "*", "/", "%", "<", ">", "=", "!", "&", "|", "^", "~",
    "(", ")", "{", "}", "[", "]", ",", ";", ":", ".", "?",
]

PASANGAN_ESCAPE = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", "\\": "\\", '"': '"', "'": "'"}


class GalatLex(Exception):
    def __init__(self, pesan, baris, kolom):
        super().__init__(f"baris {baris}:{kolom}: {pesan}")
        self.pesan = pesan
        self.baris = baris
        self.kolom = kolom


@dataclass
class Token:
    jenis: str
    nilai: object
    baris: int
    kolom: int

    def __repr__(self):
        return f"Token({self.jenis!r}, {self.nilai!r}, {self.baris}:{self.kolom})"


class Lexer:
    def __init__(self, sumber, nama="<masukan>"):
        self.s = sumber
        self.nama = nama
        self.i = 0
        self.baris = 1
        self.kolom = 1
        self.n = len(sumber)

    def _maju(self, k=1):
        for _ in range(k):
            if self.i < self.n:
                if self.s[self.i] == "\n":
                    self.baris += 1
                    self.kolom = 1
                else:
                    self.kolom += 1
                self.i += 1

    def _lihat(self, offset=0):
        j = self.i + offset
        return self.s[j] if j < self.n else ""

    def _lewati_spasi(self):
        while self.i < self.n:
            c = self.s[self.i]
            if c in " \t\r\n":
                self._maju()
            elif c == "/" and self._lihat(1) == "/":
                while self.i < self.n and self.s[self.i] != "\n":
                    self._maju()
            elif c == "/" and self._lihat(1) == "*":
                b0, k0 = self.baris, self.kolom
                self._maju(2)
                while self.i < self.n and not (self.s[self.i] == "*" and self._lihat(1) == "/"):
                    self._maju()
                if self.i >= self.n:
                    raise GalatLex("komentar blok tidak ditutup", b0, k0)
                self._maju(2)
            else:
                return

    def _baca_angka(self):
        b0, k0 = self.baris, self.kolom
        mulai = self.i
        if self.s[self.i] == "0" and self._lihat(1) in "xXbBoO":
            basis_penanda = self._lihat(1).lower()
            self._maju(2)
            awal_digit = self.i
            while self.i < self.n and (self.s[self.i].isalnum() or self.s[self.i] == "_"):
                self._maju()
            teks = self.s[awal_digit:self.i].replace("_", "")
            basis = {"x": 16, "b": 2, "o": 8}[basis_penanda]
            try:
                nilai = int(teks, basis)
            except ValueError:
                raise GalatLex(f"literal bilangan tidak sah: {self.s[mulai:self.i]!r}", b0, k0)
            return Token("INT", nilai, b0, k0)
        while self.i < self.n and (self.s[self.i].isdigit() or self.s[self.i] == "_"):
            self._maju()
        pecahan = False
        if self._lihat() == "." and self._lihat(1).isdigit():
            pecahan = True
            self._maju()
            while self.i < self.n and (self.s[self.i].isdigit() or self.s[self.i] == "_"):
                self._maju()
        if self._lihat() in "eE" and (self._lihat(1).isdigit() or (self._lihat(1) in "+-" and self._lihat(2).isdigit())):
            pecahan = True
            self._maju()
            if self._lihat() in "+-":
                self._maju()
            while self.i < self.n and self.s[self.i].isdigit():
                self._maju()
        teks = self.s[mulai:self.i].replace("_", "")
        if pecahan:
            return Token("PECAHAN", float(teks), b0, k0)
        return Token("INT", int(teks), b0, k0)

    def _baca_teks(self):
        b0, k0 = self.baris, self.kolom
        self._maju()  # buka kutip
        buf = []
        while True:
            if self.i >= self.n:
                raise GalatLex("literal teks tidak ditutup", b0, k0)
            c = self.s[self.i]
            if c == '"':
                self._maju()
                return Token("TEKS", "".join(buf), b0, k0)
            if c == "\n":
                raise GalatLex("literal teks tidak boleh melewati baris", b0, k0)
            if c == "\\":
                self._maju()
                e = self._lihat()
                if e in PASANGAN_ESCAPE:
                    buf.append(PASANGAN_ESCAPE[e])
                    self._maju()
                elif e == "x":
                    self._maju()
                    hx = self.s[self.i:self.i + 2]
                    if len(hx) != 2:
                        raise GalatLex("escape \\x perlu dua digit heksadesimal", b0, k0)
                    try:
                        buf.append(chr(int(hx, 16)))
                    except ValueError:
                        raise GalatLex(f"escape \\x tidak sah: {hx!r}", b0, k0)
                    self._maju(2)
                else:
                    raise GalatLex(f"escape tidak dikenal: \\{e}", b0, k0)
            else:
                buf.append(c)
                self._maju()

    def tokenkan(self):
        keluaran = []
        while True:
            self._lewati_spasi()
            if self.i >= self.n:
                keluaran.append(Token("EOF", None, self.baris, self.kolom))
                return keluaran
            b0, k0 = self.baris, self.kolom
            c = self.s[self.i]
            if c.isdigit():
                keluaran.append(self._baca_angka())
                continue
            if c == '"':
                keluaran.append(self._baca_teks())
                continue
            if c.isalpha() or c == "_":
                mulai = self.i
                while self.i < self.n and (self.s[self.i].isalnum() or self.s[self.i] == "_"):
                    self._maju()
                kata = self.s[mulai:self.i]
                if kata in SINONIM_ID:
                    keluaran.append(Token("ID", SINONIM_ID[kata], b0, k0))
                elif kata in _ALIAS:
                    kanonik = kata if kata in KATA_KUNCI else next(
                        k for k, v in KATA_KUNCI.items() if kata in v)
                    keluaran.append(Token(kanonik, kata, b0, k0))
                else:
                    keluaran.append(Token("ID", kata, b0, k0))
                continue
            for sim in SIMBOL:
                if self.s.startswith(sim, self.i):
                    self._maju(len(sim))
                    keluaran.append(Token(sim, sim, b0, k0))
                    break
            else:
                raise GalatLex(f"karakter tidak dikenal: {c!r}", b0, k0)


def tokenkan(sumber, nama="<masukan>"):
    return Lexer(sumber, nama).tokenkan()
