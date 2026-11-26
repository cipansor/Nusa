"""Model tipe bahasa Nusa."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Tipe:
    kind: str                 # int | float | logika | teks | kosong | deret | bentuk | pilihan
    bits: int = 0
    signed: bool = True
    elem: object = None       # Tipe (untuk deret)
    n: int = 0                # panjang deret
    nama: str = ""            # nama bentuk/pilihan
    medan: tuple = ()         # ((nama, Tipe), ...) untuk bentuk
    varian: tuple = ()        # (nama, ...) untuk pilihan


def INT(bits, signed=True):
    return Tipe("int", bits=bits, signed=signed)


LOGIKA = Tipe("logika")
PECAHAN = Tipe("float", bits=64)
PECAHAN32 = Tipe("float", bits=32)
TEKS = Tipe("teks")
KOSONG = Tipe("kosong")

DASAR = {
    "bulat": INT(64, True),
    "bulat8": INT(8, True),
    "bulat16": INT(16, True),
    "bulat32": INT(32, True),
    "bulat64": INT(64, True),
    "cacah": INT(64, False),
    "cacah8": INT(8, False),
    "cacah16": INT(16, False),
    "cacah32": INT(32, False),
    "cacah64": INT(64, False),
    "logika": LOGIKA,
    "pecahan": PECAHAN,
    "pecahan32": PECAHAN32,
    "teks": TEKS,
    "kosong": KOSONG,
    # padanan Latin
    "i8": INT(8, True), "i16": INT(16, True), "i32": INT(32, True), "i64": INT(64, True),
    "u8": INT(8, False), "u16": INT(16, False), "u32": INT(32, False), "u64": INT(64, False),
    "f32": PECAHAN32, "f64": PECAHAN,
    "bool": LOGIKA, "logikal": LOGIKA,
    "str": TEKS, "string": TEKS, "void": KOSONG,
    "int": INT(64, True), "uint": INT(64, False),
}


def deret(elem, n):
    return Tipe("deret", elem=elem, n=n)


def hitung_rentang(t):
    if t.kind != "int":
        return None
    if t.signed:
        return (-(1 << (t.bits - 1)), (1 << (t.bits - 1)) - 1)
    return (0, (1 << t.bits) - 1)


def nama_tipe(t):
    if t is None:
        return "<tak dikenal>"
    if t.kind == "int":
        for k, v in DASAR.items():
            if v == t and k not in ("i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "int", "uint"):
                return k
        return f"{'i' if t.signed else 'u'}{t.bits}"
    if t.kind == "float":
        return "pecahan" if t.bits == 64 else "pecahan32"
    if t.kind == "deret":
        return f"deret<{nama_tipe(t.elem)}, {t.n}>"
    if t.kind in ("bentuk", "pilihan"):
        return f"{t.kind} {t.nama}"
    return t.kind


def apakah_angka(t):
    return t.kind in ("int", "float")


def apakah_bulat(t):
    return t.kind == "int"
