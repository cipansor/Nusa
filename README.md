# Nusa — bahasa pemrograman Indonesia dengan backend LLVM

Nusa (dari *Nusantara*) adalah bahasa pemrograman eksperimental ber-ekstensi
`.ns`. Semua kata kunci berbahasa Indonesia dan punya **singkatan** untuk
mengetik cepat. Program Nusa bisa dijalankan dengan **dua cara**: dikompilasi
menjadi kode mesin lewat LLVM, atau langsung ditafsirkan tanpa toolchain.
Keduanya memakai pemeriksa dan semantik yang sama. Tiga tujuan utamanya:

1. **Aman** — memori aman dan tipe aman sejak kompilasi, ditambah pemeriksaan
   runtime yang tidak bisa dilewati (indeks, luapan, pembagian nol, konversi).
2. **Cepat & ringan** — kode mesin langsung, tanpa runtime besar, tanpa GC,
   tanpa `malloc` implisit. Biner hasil biasanya ~16 KB.
3. **Bisa dibuktikan** — fungsi bisa diberi kontrak `syarat`/`jamin` dan
   invarian putaran; `nusa bukti` memeriksanya dengan Z3 dan menerbitkan
   berkas SMT-LIB untuk pemeriksaan ulang oleh pihak ketiga.

> Status: eksperimen v0.1. Lihat bagian *Batasan* sebelum memakai untuk
> apa pun yang serius.

---

## 1. Pasang dan jalankan

Prasyarat: Python 3.10+ dan `llvmlite` (plus `z3-solver` untuk `nusa bukti`).
Mode penafsir hanya butuh Python — tanpa LLVM, tanpa penaut C.

```bash
pip install llvmlite z3-solver     # sekali saja

./bin/nusa jalan  contoh/fitur.ns        # mode penafsir: langsung jalan
./bin/nusa bangun contoh/fitur.ns -o fitur && ./fitur   # mode kompiler: kode mesin
./bin/nusa bukti  contoh/kontrak.ns      # verifikasi kontrak dengan Z3
```

Atau pasang sebagai perintah `nusa`:

```bash
pip install -e .
nusa bangun contoh/kontrak.ns -o kontrak
```

## 2. Perintah

| Perintah | Arti |
| --- | --- |
| `nusa jalan berkas.ns` | **penafsir**: jalankan AST langsung (alias `jalankan`, `interp`, `interpretasi`) |
| `nusa jalan berkas.ns --native` | kompilasi cepat lalu jalankan (mode kompiler) |
| `nusa bangun berkas.ns -o prog` | **kompiler**: bangun program native (alias `kompilasi`) |
| `nusa cek berkas.ns` | periksa sintaks + tipe saja (tanpa eksekusi) |
| `nusa ir berkas.ns` | cetak IR LLVM hasil optimasi |
| `nusa asm berkas.ns` | cetak assembly mesin |
| `nusa bukti berkas.ns` | verifikasi kontrak dengan Z3 |

Opsi umum: `-O0..-O3` (bawaan `-O2`), `--boleh-takaman`, `--cc`, `-s/--strip`,
`--obj berkas.o`, `--ir berkas.ll`, dan pada `bukti`: `--smt DIR`, `-v`.

### 2.1 Dua mode eksekusi

| | Penafsir (`nusa jalan`) | Kompiler (`nusa bangun`) |
| --- | --- | --- |
| Alur | AST dieksekusi langsung | AST → IR LLVM → kode mesin → `cc` |
| Butuh | Python saja | Python + `llvmlite` + penaut C |
| Waktu mulai | ~60 ms | ~125 ms (kompilasi) |
| Kecepatan program | ±5 µs per pernyataan | kecepatan kode mesin (≈ C) |
| Pemeriksaan runtime | batas indeks, luapan, bagi nol, geser, konversi | sama |
| Keluaran & pesan galat | identik dengan hasil kompilasi | acuan |

Keduanya memakai pemeriksa tipe yang sama, jadi program yang ditolak kompiler
juga ditolak penafsir — penafsir tidak pernah menjalankan program yang tidak
lolos pemeriksaan. Uji `test_semua_contoh_sama_di_kedua_mode` menjalankan setiap
contoh di kedua mode lalu membandingkan stdout, stderr, dan kode keluar,
termasuk jalur panik.

Praktiknya: penafsir untuk iterasi cepat (`nusa jalan main.ns`), kompiler untuk
rilis (`nusa bangun main.ns -o main`). Untuk membandingkan keduanya:

```bash
nusa jalan contoh/fitur.ns  > a.txt
nusa bangun contoh/fitur.ns -o fitur && ./fitur > b.txt
diff a.txt b.txt && echo "sama"
```

## 3. Contoh singkat

```ns
// setiap variabel wajib punya nilai awal; 'tp' tidak bisa diubah
fungsi fpb(a: bulat, b: bulat) -> bulat {
  ub x: bulat = a;
  ub y: bulat = b;
  selama y != 0 {
    tp sisa: bulat = x % y;
    x = y;
    y = sisa;
  }
  kembali x;
}

fungsi utama() {
  cetak("fpb(48, 36) =", fpb(48, 36));
}
```

Singkatan bekerja di mana saja — program berikut sama artinya:

```ns
fn fpb(a: bulat, b: bulat) -> bulat {
  ub x = a; ub y = b;
  slm y != 0 { tp sisa = x % y; x = y; y = sisa; }
  kmb x;
}
```

## 4. Referensi bahasa

### 4.1 Kata kunci

| Konsep | Utama | Singkatan | Padanan Latin |
| --- | --- | --- | --- |
| fungsi | `fungsi` | `fn` | – |
| variabel bisa diubah | `ubah` | `ub` | – |
| variabel tetap | `tetap` | `tp` | – |
| kembalikan nilai | `kembali` | `kmb` | `return` |
| jika / selain itu | `jika` / `lainnya` | `jk` / `ln` | `if` / `else` |
| selama (while) | `selama` | `slm` | `while` |
| untuk (for) | `untuk` | `utk` | `for` |
| dalam / dari / sampai | `dalam` / `dari` / `sampai` | `dlm` / `dr` / `sp` | `in` |
| ulangi terus | `ulang` | `ul` | `loop` |
| henti / lanjut | `henti` / `lanjut` | `ht` / `lnj` | `break` / `continue` |
| benar / salah | `benar` / `salah` | `bnr` / `slh` | `true` / `false` |
| dan / atau / bukan | `dan` / `atau` / `bukan` | – | `and` / `or` / `not` |
| bentuk (struct) | `bentuk` | `bt` | `struct` |
| pilihan (enum) | `pilihan` | `plh` | `enum` |
| cocok / kasus | `cocok` / `kasus` | `cck` / `ks` | `match` / `case` |
| alias tipe | `jenis` | `jns` | `type` |
| blok tak aman | `takaman` | `ta` | `unsafe` |
| prasyarat kontrak | `syarat` | `syr` | `requires` |
| jaminan kontrak | `jamin` | `jmn` | `ensures` |
| invarian putaran | `invarian` | `inv` | `invariant` |
| periksa (assert) | `periksa` | `prk` | `assert` |
| konversi tipe | `sebagai` | `sbg` | `as` |
| fungsi murni | `murni` | `mrn` | `pure` |
| cetak / panjang | `cetak` / `panjang` | `ctk` / `pjg` | `print` / `len` |

### 4.2 Tipe

| Tipe | Arti |
| --- | --- |
| `bulat`, `bulat8/16/32/64` | bilangan bulat bertanda (i64 bawaan) |
| `cacah`, `cacah8/16/32/64` | bilangan cacah tanpa tanda (u64 bawaan) |
| `logika` | boolean |
| `pecahan`, `pecahan32` | f64 / f32 |
| `teks` | string UTF-8 tak berubah |
| `kosong` | tanpa nilai (hasil fungsi) |
| `deret<T, N>` | larik tetap, panjang N, disimpan di stack |
| `bentuk Nama` | struct, disalin berdasarkan nilai |
| `pilihan Nama` | enum dengan `N` varian, `cocok` wajib lengkap |

Alias Latin juga diterima: `i8..i64`, `u8..u64`, `f32`, `f64`, `bool`, `str`,
`void`.

### 4.3 Pernyataan

```ns
tp x: bulat = 10;            // tetap, wajib diberi nilai awal
ub y = 0;                    // ubah, tipe disimpulkan (bulat)
y = y + 1;                   // penugasan; juga += -= *= /= %= &= |= ^=
jika y > 3 { } lainnya jika y == 3 { } lainnya { }
untuk i dalam 0..10 { }      // 0..9  (eksklusif)
untuk i dalam 0..=10 { }     // 0..10 (inklusif)
untuk i dari 0 sampai 10 { } // 0..10 (gaya bahasa Indonesia)
selama y > 0 { y -= 1; }
ulang { henti; }
cocok arah {
  kasus Utara   { }
  kasus Selatan { }
  lainnya       { }
}
periksa y >= 0, "y tidak boleh negatif";
{ tp sementara: bulat = 1; }  // blok berskop sendiri
```

### 4.4 Ekspresi

Aritmetika `+ - * / %`, perbandingan `== != < <= > >=`, logika `dan/atau/bukan`
(atau `&& || !`), bit `& | ^ ~ << >>`, panggilan `f(a, b)`, indeks `d[i]`,
medan `t.x`, literal deret `[1, 2, 3]`, literal bentuk `Titik { x: 1, y: 2 }`,
dan konversi `nilai sebagai tipe`.

Operator logika **memendek** (short-circuit), jadi
`jika n != 0 dan 100 / n > 2 { }` tidak akan panik saat `n = 0`.

## 5. Model keamanan

Tidak ada pointer yang bisa diaritmetikkan, tidak ada `null`, tidak ada memori
yang tak terinisialisasi (setiap deklarasi wajib punya nilai awal), dan tidak
ada `free` manual.

| Kesalahan | Kapan terdeteksi | Akibat |
| --- | --- | --- |
| menulis ke `tetap` / parameter | kompilasi | galat tipe |
| tipe tidak cocok / campur `bulat` dan `cacah` | kompilasi | galat tipe |
| literal di luar rentang tipe | kompilasi | galat tipe |
| `cocok` tidak lengkap pada pilihan | kompilasi | galat tipe |
| fungsi tidak mengembalikan nilai di semua jalur | kompilasi | galat tipe |
| nilai dipakai sebelum dideklarasikan | kompilasi | nama tak dikenal |
| indeks di luar batas `0..N-1` | runtime | panik + baris |
| luapan `+ - *` dan negasi | runtime | panik + baris |
| pembagian / sisa dengan nol, `MIN / -1` | runtime | panik + baris |
| geser ≥ lebar bit | runtime | panik + baris |
| `pecahan sebagai bulat` di luar rentang | runtime | panik + baris |

Pemeriksaan runtime yang **tidak perlu** dibuang oleh optimizer. Contohnya,
pada `untuk i dalam 0..panjang(d) { d[i] }` batas indeks terbukti dari bentuk
putaran, sehingga tidak ada pemeriksaan sama sekali di kode mesin.

Blok `takaman { ... }` mematikan pemeriksaan di dalamnya (aritmetika menjadi
*wrapping*, indeks tidak dicek). Blok ini **ditolak** kecuali kompilasi memakai
`--boleh-takaman`, dan setiap pemakaiannya memunculkan peringatan.

## 6. Verifikasi formal

Fungsi `murni` boleh membawa kontrak:

```ns
murni fungsi hitung_naik(n: bulat) -> bulat
  syarat n >= 0
  syarat n <= 1000000
  jamin hasil == n
{
  ub x: bulat = 0;
  selama x < n
    invarian 0 <= x
    invarian x <= n
  {
    x += 1;
  }
  kembali x;
}
```

```console
$ ./bin/nusa bukti contoh/kontrak.ns
bukti: contoh/kontrak.ns
  tambah_aman  [TERBUKTI]  3 obligasi, 1 jalur kembali
  mutlak       [TERBUKTI]  5 obligasi, 2 jalur kembali
  hitung_naik  [TERBUKTI]  7 obligasi, 1 jalur kembali
ringkasan: 3 terbukti, 0 terbantah, 0 tidak pasti
```

Yang diperiksa:

* `syarat` — diasumsikan saat masuk fungsi, dan **dibuktikan** di setiap
  pemanggilan ke fungsi `murni` lain yang berkontrak.
* `jamin` — dibuktikan untuk setiap jalur `kembali`, dengan `hasil` sebagai
  nilai kembalian.
* `periksa` — dibuktikan selalu benar pada titik itu.
* **bebas luapan** — setiap `+ - *` dan konversi menghasilkan kewajiban rentang,
  jadi program yang bisa meluap ditolak, bukan "dianggap benar".
* **invarian putaran** — dibuktikan (a) saat masuk putaran, (b) terjaga oleh
  badan putaran (langkah induktif dengan nilai sembarang yang memenuhi
  invarian), dan (c) dipakai untuk menyimpulkan keadaan setelah putaran.

Keluaran status: `TERBUKTI`, `TERBANTAH` (dengan contoh penyangkal, misalnya
`mutlak_x = -9223372036854775808`), atau `TIDAK PASTI`. Kode keluar: `0` semua
terbukti, `3` ada yang terbantah, `2` ada yang tidak pasti.

Dengan `--smt DIR`, setiap kewajiban juga ditulis sebagai berkas SMT-LIB2:

```smt
(set-logic QF_LIA)
(declare-fun hitung_naik_n () Int)
(assert (and (>= hitung_naik_n -9223372036854775808) (<= hitung_naik_n 9223372036854775807)))
(assert ...)
(check-sat)
```

Berkas itu bisa diperiksa ulang oleh `z3`, `cvc5`, atau alat pihak ketiga
lainnya — penting untuk audit sistem kritis.

### Batasan verifikasi (disengaja agar tetap *sound*)

* Aritmetika dimodelkan sebagai bilangan matematis + kewajiban rentang.
* Pembagian/sisa **bilangan bulat** tidak dimodelkan (pembulatan mesin ≠
  aritmetika eksak). Gunakan `pecahan` atau bantu dengan `periksa`.
* Belum didukung: rekursi, akses deret, medan bentuk, dan teks di dalam
  kontrak. Semuanya dilaporkan `TIDAK PASTI`, tidak pernah "terbukti" palsu.
* Putaran wajib punya `invarian`; tanpa itu statusnya `TIDAK PASTI`.

## 7. Kinerja dan ukuran

Diukur pada satu inti (AMD EPYC 9B14), `-O2`, median 5 kali jalan
(`python3 tolok/ukur.py`):

| Tolok ukur | Nusa | C (`gcc -O2`) | Rasio |
| --- | --- | --- | --- |
| putaran skalar bergantung data, 20 juta iterasi | 18,6 ms | 18,5 ms | **1,00×** |
| jumlah deret 1024 × 10.000 (10,24 juta baca berbatas-cek) | 6,8 ms | 2,3 ms | 3,0× |

Mode penafsir tentu lebih lambat: 1 juta iterasi putaran ±4,9 detik (±5 µs per
pernyataan), sekitar 2.700× lebih lambat daripada biner. Itu harga wajar untuk
eksekusi AST murni-Python; gunakan `nusa bangun` untuk beban berat.

Pemeriksaan runtime praktis gratis saat cabangnya mudah diprediksi; selisih
pada tolok ukur deret berasal dari vektorisasi otomatis yang dilakukan gcc
(loop Nusa belum divektorisasi).

Ukuran: program "halo" menjadi **~16 KB** biner (objek ~2 KB), tanpa runtime
tambahan — `printf`, `strlen`, `fprintf`, dan `abort` diambil dari libc.
Biner tolok ukur deret 28 KB karena memuat literal 1024 elemen.

## 8. Struktur kode

| Berkas | Isi |
| --- | --- |
| `nusa/lexer.py` | tokenisasi, kata kunci + singkatan |
| `nusa/ast.py` | simpul AST |
| `nusa/parser.py` | parser rekursif-turun |
| `nusa/tipe.py` | model tipe dan rentang |
| `nusa/periksa.py` | pemeriksa tipe/mutabilitas/alur |
| `nusa/codegen.py` | pembangkit IR LLVM + pemeriksaan runtime |
| `nusa/interp.py` | penafsir AST (mode `nusa jalan`) |
| `nusa/bukti.py` | verifikasi kontrak dengan Z3 + ekspor SMT-LIB |
| `nusa/cli.py` | antarmuka perintah, optimasi, penautan |
| `contoh/*.ns` | program contoh |
| `uji/uji_nusa.py` | 47 uji otomatis (termasuk uji diferensial penafsir vs kompiler) |
| `tolok/ukur.py` | tolok ukur Nusa vs C |

Alur kompiler: `lexer → parser → periksa → codegen → optimasi LLVM → objek → cc`.
Alur penafsir: `lexer → parser → periksa → interp`.

## 9. Uji

```bash
python3 -m unittest discover -s uji -p "uji_*.py" -v
```

Uji mencakup keluaran program, penolakan tipe, panik runtime (batas, luapan,
bagi nol, geser, konversi), `takaman`, verifikasi terbukti/terbantah/tidak
pasti, ekspor SMT, ukuran biner, dan **uji diferensial**: setiap contoh dan
beberapa program "torture" dijalankan lewat penafsir dan lewat biner hasil
kompilasi, lalu stdout/stderr/kode keluarnya harus sama persis.

## 10. Batasan dan peta jalan

Sudah ada: tipe dasar, deret tetap, bentuk, pilihan + `cocok` lengkap, putaran,
rekursi, kontrak + invarian, pemeriksaan runtime, `takaman`, ekspor SMT.

Belum ada (urutan usulan):

1. `opsional<T>` sebagai pengganti `null`, plus `?`/`!`.
2. Referensi `pinjam` dengan aturan masa hidup (borrow checker ringan).
3. `deret` dinamis / potongan (`irisan<T>`) dengan pemeriksaan batas.
4. `map`/`filter` gaya iterator, modul dan impor, generik monoton.
5. Penghapusan pemeriksaan luapan lewat analisis rentang (seperti batas indeks).
6. Penafsir: kompilasi AST ke closure/bytecode agar 5–10× lebih cepat, plus
   mode `--ulang` (REPL) dan pelacakan langkah.
7. Verifikasi: rekursi dengan varian, akses deret/medan, pembagian bulat eksak.

## 11. Mengapa desainnya begini

* **Kata kunci Indonesia + singkatan** — `fungsi`/`fn`, `selama`/`slm`: mudah
  dibaca pemula, tetap cepat diketik.
* **Immutabilitas bawaan** (`tetap`) dan mutasi eksplisit (`ubah`) — menghapus
  satu kelas bug tanpa biaya runtime.
* **Tanpa pointer terlihat** — batas aman/cepat jelas: kode aman selalu
  diperiksa; `takaman` adalah pintu keluar yang harus diminta.
* **Verifikasi sebagai fitur bahasa**, bukan alat tempelan — kontrak ada di
  dekat kode, dan hasilnya bisa diaudit lewat SMT-LIB.
* **Dua mode eksekusi** — penafsir untuk kecepatan pengembangan (tanpa
  toolchain), kompiler untuk kecepatan program. Karena keduanya berbagi AST
  bertipe dan pemeriksa yang sama, tidak ada "bahasa kedua" yang harus
  dipelajari atau diselaraskan manual.

## 12. Lisensi

Apache License 2.0 — lihat berkas [LICENSE](LICENSE).
