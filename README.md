# Nusa (.ns)

**Bahasa pemrograman berbahasa Indonesia, ditulis dalam Rust, dengan backend LLVM dan interpreter langsung.**
Aman memori, bertipe statis, cepat, hemat memori, dan dapat diverifikasi formal. Berkas sumber selalu berekstensi **`.ns`**.

```
fungsi utama() -> kosong {
    cetakln("Halo, Nusa!")
}
```

Nusa adalah eksperimen bahasa yang mengejar lima hal sekaligus:

| Tujuan | Cara Nusa mencapainya |
|---|---|
| **Aman memori** | Tidak ada pointer/null bagi pengguna; batas daftar selalu diperiksa; objek heap dikelola penghitung referensi + *copy-on-write*; tanpa siklus referensi. |
| **Aman tipe** | Pemeriksa tipe statis, tanpa konversi implisit (kecuali literal bulat → `des`), variabel wajib diberi nilai awal, `tetap` tidak dapat diubah. |
| **Cepat** | Dua mode eksekusi: **compiler** (LLVM IR + `-O2`/`-O3`) atau **interpreter** langsung; skalar di stack/register; tidak ada *garbage collector*. |
| **Rendah memori** | Tanpa runtime besar; biner program ± 15–20 KB; objek heap dibebaskan tepat saat keluar lingkup; COW menghindari salinan sia-sia. |
| **Terverifikasi formal** | Kontrak `syarat`/`jamin`, pernyataan `tegaskan`, pembangkitan *verification condition* ke SMT-LIB, dibuktikan dengan z3. |
| **Ergonomis** | Kata kunci & singkatan bahasa Indonesia, sintaks sederhana ala Python, diagnostik berbahasa Indonesia, satu berkas = satu program. |

---

## 1. Status

- Kompiler + interpreter: **Rust murni, tanpa dependensi crate** (build cepat, biner kompiler ± 700 KB).
- Backend kompilasi: **LLVM** melalui `clang` (IR dihasilkan sendiri, lalu dioptimalkan & ditautkan LLVM).
- Interpreter: *tree-walking* atas AST, tanpa `clang`/LLVM; semantik, panic, dan format keluaran disamakan dengan hasil kompilasi.
- Uji: **32 uji** (`cargo test`) — unit lexer/parser/tipe, *end-to-end* kompilasi, interpreter, paritas compiler↔interpreter, panic, dan verifikasi z3.
- Contoh siap jalan di `contoh/`, termasuk `kritis.ns` (gaya sistem kritis dengan kontrak terbukti).

Belum ada di v0.1: struct, generik, modul, closure, pointer, dan pembuktian lintas-fungsi. Lihat [Roadmap](#12-roadmap).

---

## 2. Instalasi

Prasyarat:

1. **Rust** (stable) — untuk membangun kompiler.
2. **LLVM/clang** — hanya untuk **mode compiler**. Di Debian/Ubuntu:
   `sudo apt install clang lld`. Mode `interpretasi` tidak memerlukannya.
3. **z3** (opsional) — untuk membuktikan kontrak:
   `sudo apt install z3`

Membangun kompiler:

```bash
cargo build --release
./target/release/nusa versi
```

Tidak ada dependensi crate, jadi `cargo build` hanya memerlukan Rust. Kompiler memanggil `clang-19`, `clang`, atau `cc` (dapat diatur lewat `NUSA_CC`).

---

## 3. Mulai cepat

```bash
# mode compiler: kompilasi ke biner native lalu jalankan
./target/release/nusa jalankan contoh/halo.ns

# mode interpreter: jalankan langsung tanpa clang/LLVM
./target/release/nusa interpretasi contoh/halo.ns
./target/release/nusa jalankan contoh/halo.ns --tafsir

# bangun biner
./target/release/nusa bangun contoh/faktorial.ns -o faktorial
./faktorial

# periksa tipe & keamanan tanpa membangun
./target/release/nusa periksa contoh/prima.ns

# lihat LLVM IR
./target/release/nusa ir contoh/fibonacci.ns

# buktikan kontrak dengan z3
./target/release/nusa verifikasi contoh/pembuktian.ns
./target/release/nusa verifikasi contoh/kritis.ns
```

### 3.1 Dua mode eksekusi

| Aspek | Compiler (`jalankan`, `bangun`) | Interpreter (`interpretasi`, `--tafsir`) |
|---|---|---|
| Mesin | AST → LLVM IR → biner native | AST dijalankan langsung oleh Rust |
| Kebutuhan | `clang`/LLVM | hanya biner `nusa` |
| Kecepatan | native, dioptimalkan `-O2`/`-O3` | lebih lambat; cocok untuk prototipe, pengujian, dan lingkungan tanpa LLVM |
| Pemeriksaan | tipe, batas daftar, luapan, kontrak, panic | sama persis |
| Verifikasi formal | `nusa verifikasi` (front-end sama) | `nusa verifikasi` (front-end sama) |
| Ekstensi sumber | `.ns` | `.ns` |

Keduanya memakai *front-end* yang sama (lexer → parser → pemeriksa tipe), sehingga program yang ditolak compiler juga ditolak interpreter, dan sebaliknya. Uji paritas di `tests/interpreter.rs` membandingkan **stdout, stderr, dan kode keluar** kedua mode untuk semua contoh dan kasus panic. Interpreter membatasi kedalaman pemanggilan 2000 dengan panic terkendali, sedangkan mode compiler mengikuti batas stack native.

---

## 4. Sekilas bahasa

```ns
# Komentar diawali # (atau //)

# Fungsi + kontrak
fungsi bagi(a: bil, b: bil) -> bil
    syarat a >= 0 dan b > 0
    jamin hasil >= 0 dan hasil <= a
{
    kembalikan a / b
}

fungsi jumlah(d: daftar<bil>) -> bil {
    var total: bil = 0
    untuk x dalam d {
        total += x
    }
    kembalikan total
}

fungsi utama() -> kosong {
    var d: daftar<bil> = [3, 1, 4]
    d.tambah(5)
    cetakln("d = ", d)
    cetakln("jumlah = ", jumlah(d))
    cetakln(bagi(17, 5))

    var s: teks = "Nusa"
    s += " kuat"
    cetakln(s, " (", panjang(s), " bita)")

    var i: bil = 0
    selama i < 5 {
        i += 1
        jika i == 2 { lanjutkan }
        jika i == 4 { hentikan }
        cetak(i, " ")
    }
    cetakln("")
}
```

Keluaran:

```
d = [3, 1, 4, 5]
jumlah = 13
3
Nusa kuat (9 bita)
1 3
```

---

## 5. Referensi bahasa

### 5.1 Kata kunci dan singkatan

Setiap kata kunci punya singkatan Indonesia yang setara. Singkatan dipakai agar penulisan cepat tanpa kehilangan keterbacaan.

| Lengkap | Singkatan | Arti |
|---|---|---|
| `fungsi` | `fng` | function |
| `kembalikan` | `kmb` | return |
| `jika` | `jk` | if |
| `lainnya` | `lain`, `ln` | else |
| `selama` | `slm` | while |
| `untuk` | `utk` | for |
| `dalam` | `dlm` | in |
| `tetap` | `ttp` | let / const (tidak dapat diubah) |
| `var` | — | variabel yang dapat diubah |
| `dan` | `dn`, `&&` | and |
| `atau` | `at`, `\|\|` | or |
| `bukan` | `bkn`, `!` | not |
| `benar` / `salah` | `bn` / `sl` | true / false |
| `bil` | `bilangan` | bilangan bulat (i64) |
| `des` | `desimal` | desimal (f64) |
| `log` | `logika` | logika (bool) |
| `teks` | — | string UTF-8 |
| `daftar<T>` | `dft<T>` | daftar dinamis |
| `kosong` | `ksg` | unit / void |
| `syarat` | `syr` | prasyarat kontrak |
| `jamin` | `jmn` | pascakondisi kontrak |
| `invarian` | `inv` | invarian loop (untuk verifikasi) |
| `tegaskan` | `tgs` | assertion saat berjalan |
| `hentikan` | `htk` | break |
| `lanjutkan` | `lnj` | continue |

`hasil` adalah nama khusus di dalam `jamin` yang merujuk nilai kembalian.

### 5.2 Tipe

| Tipe | Representasi | Contoh |
|---|---|---|
| `bil` | i64 | `42`, `-7`, `0x2A`, `0b1010`, `1_000` |
| `des` | f64 | `3.14`, `1.0e9` |
| `log` | i1 | `benar`, `salah` |
| `teks` | UTF-8, dikelola RC | `"halo\n"`, `"\u{1F600}"` |
| `daftar<T>` | elemen 8 bita, dikelola RC | `[1, 2, 3]`, `["a", "b"]` |
| `kosong` | unit | hasil fungsi tanpa nilai |

Daftar hanya boleh berisi tipe skalar, `teks`, atau `daftar<T>` lain (bukan struct — belum ada di v0.1). Daftar kosong memerlukan anotasi tipe: `var d: daftar<bil> = []`.

### 5.3 Variabel dan penugasan

```ns
tetap pi: des = 3.14       # tidak dapat diubah
var hitung: bil = 0        # dapat diubah
hitung = hitung + 1
hitung += 5                # += -= *= /= %=
var d: daftar<bil> = [1, 2]
d[0] = 9                   # indeks selalu diperiksa batasnya
d.tambah(3)                # metode daftar
```

Semua variabel wajib diberi nilai awal. Tidak ada nilai *null* atau variabel tak terinisialisasi.

### 5.4 Kendali alur

```ns
jika x > 0 {
    cetakln("positif")
} lainnya jika x == 0 {
    cetakln("nol")
} lainnya {
    cetakln("negatif")
}

selama kondisi invarian k1 dan k2 {
    ...
}

untuk i dalam 0..10 {      # 0 sampai 9
    ...
}
untuk i dalam 0..=10 {     # 0 sampai 10
    ...
}
untuk x dalam daftar {      # setiap elemen
    ...
}
```

`invarian` opsional saat eksekusi, tetapi **wajib** untuk verifikasi loop `selama`.

### 5.5 Operator

| Prioritas | Operator |
|---|---|
| 1 (rendah) | `atau`, `\|\|` |
| 2 | `dan`, `&&` |
| 3 | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| 4 | `+`, `-` |
| 5 | `*`, `/`, `%` |
| 6 (tinggi) | unary `-`, `bukan`, `!` |

- `+` juga menggabungkan `teks`.
- `==`/`!=` berlaku untuk `bil`, `des`, `log`, `teks`; `<` dll. juga untuk `teks` (urutan leksikografis).
- `/` dan `%` bilangan bulat memakai **pembagian Euclidean**: hasil `%` selalu `0 <= r < |b|`.
  Contoh: `-7 / 3 == -3`, `-7 % 3 == 2`, `7 % -3 == 1`.
- Tidak ada konversi implisit; literal bulat boleh dipakai di konteks `des` (`1.5 * 2`).

### 5.6 Fungsi bawaan

| Bawaan | Keterangan |
|---|---|
| `cetak(nilai, ...)` | cetak tanpa baris baru; menerima semua tipe yang dapat dicetak, termasuk daftar bersarang |
| `cetakln(nilai, ...)` | seperti `cetak` + baris baru |
| `panjang(x)` | panjang `teks` dalam bita (UTF-8) atau jumlah elemen `daftar` |
| `mutlak(x)` | nilai absolut `bil`/`des` |
| `d.tambah(x)` | tambah elemen ke daftar (daftar harus `var`) |
| `d.panjang()` | panjang daftar |

---

## 6. Keamanan memori

Nusa tidak mengekspos pointer. Model memorinya:

1. **Nilai, bukan referensi.** Assignment dan pemberian argumen memiliki semantik nilai: mutasi pada salinan tidak terlihat oleh pemilik asli.
2. **Copy-on-write.** Daftar/teks memakai penghitung referensi. Mutasi pada objek yang dipakai bersama menyalin lebih dulu; bila referensi tunggal, mutasi dilakukan di tempat.
3. **Bebas saat keluar lingkup.** Kompiler menyisipkan `release` tepat di akhir blok, pada `hentikan`/`lanjutkan`/`kembalikan`, dan di blok keluar fungsi (idempoten, aman dari *double free* karena slot dinullkan).
4. **Tanpa siklus.** Tidak ada referensi ke diri sendiri dan tipe rekursif tidak ada, sehingga penghitung referensi tidak pernah membentuk siklus bocor.
5. **Batas selalu diperiksa.** `d[i]` memanggil pemeriksaan batas di runtime, bahkan pada `--lepas`. Indeks negatif/terlalu besar memicu panic yang terkendali (bukan akses liar).
6. **Aritmetika aman.** Secara bawaan `+`, `-`, `*`, negasi, dan penghitung loop diperiksa luapannya; pembagian/sisa memeriksa nol dan `MIN / -1`. Kegagalan memicu panic, bukan perilaku tak terdefinisi.
7. **Pemeriksaan runtime yang eksplisit.** `syarat`/`jamin`/`tegaskan` diperiksa saat berjalan (dapat dimatikan dengan `--tanpa-kontrak`/`--lepas`).

Karena tidak ada pointer, *use-after-free*, *double free*, *null dereference*, dan *out-of-bounds* tidak mungkin terjadi pada program Nusa yang lolos pemeriksaan tipe.

> Catatan: pemeriksaan luapan dan kontrak dapat dimatikan untuk kecepatan maksimum. Saat dimatikan, aritmetika `bil` membungkus (wrap-around) seperti integer dua-komplemen; keamanan memori tetap terjaga karena batas daftar tidak pernah dimatikan.

---

## 7. Verifikasi formal

### 7.1 Menulis kontrak

```ns
fungsi tambah_loop(n: bil) -> bil
    syarat n >= 0
    jamin hasil >= 0
{
    var t: bil = 0
    var i: bil = 0
    selama i < n invarian t >= 0 dan i >= 0 dan i <= n {
        i += 1
        t += 1
    }
    kembalikan t
}
```

`nusa verifikasi` membangkitkan *verification condition* (VC) untuk:

- setiap `jamin` pada setiap jalur `kembalikan`,
- setiap `tegaskan`,
- invarian awal dan invarian yang terjaga pada loop,
- invarian saat `hentikan`.

Badan loop diperiksa dari keadaan **sembarang** yang memenuhi invarian (variabel yang diubah di-*havoc* lebih dahulu), sehingga bukti tidak hanya berlaku untuk iterasi pertama. VC diterjemahkan ke **SMT-LIB** dan dibuktikan dengan z3 (`unsat` = terbukti). Berkas bukti ditulis ke `bukti/<fungsi>.<n>.smt2` agar dapat diaudit ulang.

Contoh keluaran:

```
verifikasi formal: contoh/pembuktian.ns
  fungsi `tambah_satu`: TERBUKTI
    [TERBUKTI] jamin: hasil >= 1
  fungsi `maks`: TERBUKTI
    [TERBUKTI] jamin: hasil >= a dan hasil >= b
    [TERBUKTI] jamin: hasil >= a dan hasil >= b
  fungsi `tambah_loop`: TERBUKTI
    [TERBUKTI] invarian awal: t >= 0 dan i >= 0 dan i <= n
    [TERBUKTI] invarian terjaga: t >= 0 dan i >= 0 dan i <= n
    [TERBUKTI] jamin: hasil >= 0
  fungsi `keliru`: GAGAL
    [GAGAL] jamin: hasil > 0
        model: ((define-fun v_a_0 () Int 0) (define-fun v_hasil_1 () Int 0))
```

Jika kontrak tidak berlaku, z3 memberi model *counterexample* (mis. `a = 0`), sehingga kesalahan mudah dilacak.

### 7.2 Batasan verifikasi v0.1

- Hanya `bil` dan `log`; `des`, `teks`, dan `daftar` belum dimodelkan.
- Fungsi yang memanggil fungsi lain dilewati (belum ada ringkasan fungsi/induksi).
- `selama` memerlukan `invarian`; `untuk rentang` mendapat invarian otomatis `mulai <= i <= akhir`.
- `untuk ... dalam daftar` belum dapat diverifikasi.
- Mutasi elemen daftar bersarang (`d[0].tambah(x)`) belum didukung; salin elemen ke variabel `var`, ubah, lalu tugaskan kembali.
- `bil` dimodelkan sebagai bilangan bulat matematis. Pemeriksaan luapan runtime menjamin tidak ada nilai salah: bila luapan, program berhenti. Karena itu bukti berlaku untuk eksekusi yang tidak panic.

Berkas yang dilewati tetap aman memori (semua pemeriksaan runtime aktif); verifikasi hanya menambah jaminan fungsional.

---

## 8. Kinerja dan ukuran (terukur di lingkungan pengembangan)

| Metrik | Nusa | C (clang) |
|---|---|---|
| `fib(32)` rekursif, `--lepas` (`-O3`) | 6,9 ms | 5,9 ms |
| `fib(32)` bawaan (cek luapan + kontrak) | 12,4 ms | — |
| Ukuran biner `halo` | 14,6 KB | ± 16 KB |
| Ukuran biner `fib` | 18,7 KB | 16,0 KB |
| Waktu kompilasi satu program | ± 0,18 dtk | ± 0,15 dtk |
| Waktu build kompiler dari nol | ± 6,5 dtk | — |
| Ukuran biner kompiler | ± 700 KB | — |

Catatan:

- Angka di atas adalah **mode compiler**. Mode interpreter mementingkan kemudahan dan paritas semantik, bukan kecepatan; gunakan `bangun`/`jalankan` untuk produksi.
- Selisih ± 17% pada `--lepas` berasal dari konvensi pemanggilan/atribut yang belum dioptimalkan (mis. tidak memakai `nsw`, belum ada inlining lintas modul).
- Mode bawaan lebih lambat karena setiap operasi memeriksa luapan dan setiap panggilan memeriksa kontrak — harga dari jaminan keselamatan.
- Loop 500.000 iterasi yang mengalokasikan `teks` + `daftar` setiap iterasi berjalan dengan RSS stabil (± 10 MB, tidak tumbuh), menunjukkan pembebasan tepat waktu.

---

## 9. CLI

```
nusa <perintah> <berkas.ns> [opsi]

Perintah:
  bangun      kompilasi .ns menjadi biner
  jalankan    kompilasi lalu jalankan (mode compiler)
  interpretasi jalankan langsung tanpa LLVM (mode interpreter)
  periksa     periksa sintaks, tipe, dan keamanan
  ir          cetak LLVM IR
  verifikasi  buktikan kontrak dengan SMT/z3
  bantuan     bantuan

Opsi:
  -o, --keluaran <jalan>   lokasi biner
  --lepas                  -O3, tanpa cek kontrak & luapan
  --tanpa-kontrak          matikan cek kontrak saat berjalan
  --tanpa-aman             matikan cek luapan aritmetika
  --tafsir                 pada `jalankan`: pakai interpreter
  -v, --verbose            tampilkan perintah tautan

Opsi verifikasi:
  --tanpa-z3               hanya tulis berkas SMT
  --keluaran-dir <dir>     direktori berkas bukti (default: bukti)
```

Variabel lingkungan: `NUSA_CC` untuk memilih `clang` (mis. `NUSA_CC=clang-18`).

---

## 10. Arsitektur

```
sumber .ns
   │
   ▼
lexer.rs     → token (kata kunci Indonesia + singkatan)
   │
   ▼
parser.rs    → AST (ast.rs) + diagnostik berbahasa Indonesia
   │
   ▼
types.rs     → pemeriksaan tipe, mutabilitas, kontrak, kepastian return
   │
   ├──────────────► verify.rs → SMT-LIB → z3 (bukti formal)
   ├──────────────► interp.rs → eksekusi AST langsung (mode interpreter)
   ▼
codegen.rs   → LLVM IR (alloca + mem2reg oleh LLVM)
   │
   ▼
clang        → objek + tautan (runtime C tertanam: src/runtime/nusa_rt.c)
   │
   ▼
biner native
```

Runtime C kecil (± 250 baris) menyediakan: alokasi/RC, string, daftar COW, pencetakan, panic, dan pembagian Euclidean. Runtime disisipkan ke dalam kompiler (`include_str!`) sehingga tidak ada berkas pendamping saat distribusi.

---

## 11. Struktur repositori

```
src/
  main.rs        CLI
  lib.rs         API kompilasi + interpreter
  diag.rs        diagnostik + render kutipan sumber
  lexer.rs       tokenizer
  ast.rs         AST
  parser.rs      parser rekursif-turun
  types.rs       pemeriksa tipe & keamanan
  codegen.rs     pembangkit LLVM IR
  interp.rs      interpreter tree-walking (semantik = hasil kompilasi)
  verify.rs      VC + SMT-LIB + z3
  driver.rs      pemanggilan clang & tautan
  runtime/
    nusa_rt.c    runtime tertanam
contoh/          program contoh
tests/e2e.rs     uji end-to-end compiler
tests/interpreter.rs uji interpreter + paritas compiler↔interpreter
README.md
```

---

## 12. Roadmap

- Struct/record dan enum, lalu pola `cocokkan` (match).
- Generik terbatas dan fungsi orde-tinggi (closure).
- Modul (`pakai`) dan paket.
- Parameter `pinjam` (borrow) untuk menghindari salinan/COW saat membaca.
- Mutasi elemen daftar bersarang dan pengindeksan/irisan `teks`.
- Verifikasi lintas-fungsi dengan ringkasan kontrak dan induksi untuk rekursi.
- Dukungan `des`/`teks`/`daftar` dalam verifikasi.
- Target non-x86 (wasm, ARM) melalui LLVM.
- Format kode (`nusa rapikan`) dan LSP.

---

## 13. Pengujian

```bash
cargo test            # 32 uji: unit + end-to-end + interpreter + z3
cargo build --release
```

Uji end-to-end/paritas akan dilewati otomatis bila `clang` atau `z3` tidak tersedia; uji interpreter murni tetap berjalan tanpa LLVM.

## 14. Lisensi

Apache License 2.0 (lihat `LICENSE`).
