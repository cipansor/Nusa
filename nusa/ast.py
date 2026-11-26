"""Simpul AST bahasa Nusa."""
from dataclasses import dataclass, field


@dataclass
class Simpul:
    baris: int = 0


# ---------- tipe ----------
@dataclass
class TipeRef(Simpul):
    nama: str = ""
    args: list = field(default_factory=list)  # TipeRef dan/atau Angka


# ---------- ekspresi ----------
@dataclass
class Angka(Simpul):
    nilai: int = 0


@dataclass
class Pecahan(Simpul):
    nilai: float = 0.0


@dataclass
class Teks(Simpul):
    nilai: str = ""


@dataclass
class Logika(Simpul):
    nilai: bool = False


@dataclass
class Nama(Simpul):
    nama: str = ""


@dataclass
class Biner(Simpul):
    op: str = ""
    kiri: Simpul = None
    kanan: Simpul = None


@dataclass
class Unari(Simpul):
    op: str = ""
    operand: Simpul = None


@dataclass
class Panggil(Simpul):
    fungsi: Simpul = None
    argumen: list = field(default_factory=list)


@dataclass
class Indeks(Simpul):
    dasar: Simpul = None
    indeks: Simpul = None


@dataclass
class Medan(Simpul):
    dasar: Simpul = None
    nama: str = ""


@dataclass
class Struktur(Simpul):
    nama: str = ""
    medan: list = field(default_factory=list)  # [(nama, ekspresi)]


@dataclass
class Sbg(Simpul):
    nilai: Simpul = None
    tipe: TipeRef = None


@dataclass
class Deret(Simpul):
    elemen: list = field(default_factory=list)


# ---------- pernyataan ----------
@dataclass
class Blok(Simpul):
    isi: list = field(default_factory=list)


@dataclass
class Dekl(Simpul):
    nama: str = ""
    tipe: TipeRef = None
    nilai: Simpul = None
    ubah: bool = False


@dataclass
class Rekah(Simpul):
    sasaran: Simpul = None
    op: str = "="
    nilai: Simpul = None


@dataclass
class Jika(Simpul):
    syarat: Simpul = None
    maka: Blok = None
    lainnya: Simpul = None  # Blok atau Jika


@dataclass
class Selama(Simpul):
    syarat: Simpul = None
    badan: Blok = None
    invarian: list = field(default_factory=list)


@dataclass
class Untuk(Simpul):
    nama: str = ""
    awal: Simpul = None
    akhir: Simpul = None
    inklusif: bool = False
    badan: Blok = None
    invarian: list = field(default_factory=list)


@dataclass
class Ulang(Simpul):
    badan: Blok = None


@dataclass
class Henti(Simpul):
    pass


@dataclass
class Lanjut(Simpul):
    pass


@dataclass
class Kembali(Simpul):
    nilai: Simpul = None


@dataclass
class Periksa(Simpul):
    syarat: Simpul = None
    pesan: str = ""


@dataclass
class Takaman(Simpul):
    badan: Blok = None


@dataclass
class Pola(Simpul):
    jenis: str = "lainnya"  # 'varian' | 'angka' | 'lainnya'
    nilai: object = None


@dataclass
class Cocok(Simpul):
    nilai: Simpul = None
    kasus: list = field(default_factory=list)  # [(Pola, Blok)]
    lainnya: Blok = None


@dataclass
class EkspresiStmt(Simpul):
    ekspresi: Simpul = None


# ---------- deklarasi tingkat atas ----------
@dataclass
class Param(Simpul):
    nama: str = ""
    tipe: TipeRef = None


@dataclass
class Fn(Simpul):
    nama: str = ""
    param: list = field(default_factory=list)
    hasil: TipeRef = None
    syarat: list = field(default_factory=list)
    jamin: list = field(default_factory=list)
    invarian: list = field(default_factory=list)
    murni: bool = False
    badan: Blok = None


@dataclass
class Bentuk(Simpul):
    nama: str = ""
    medan: list = field(default_factory=list)  # [(nama, TipeRef)]


@dataclass
class Pilihan(Simpul):
    nama: str = ""
    varian: list = field(default_factory=list)  # [nama]


@dataclass
class Jenis(Simpul):
    nama: str = ""
    tipe: TipeRef = None


@dataclass
class Program(Simpul):
    butir: list = field(default_factory=list)
