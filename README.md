# Nusa

**Bahasa pemrograman sistem berbahasa Indonesia dengan backend LLVM —
bisa dikompilasi *dan* ditafsirkan, dari sumber `berkas.ns` yang sama.**

Nusa dirancang untuk satu tujuan: membuat perangkat lunak sistem yang **aman
memori**, **aman tipe**, **cepat**, **kecil**, dan **dapat diverifikasi formal** —
tanpa mengorbankan kemudahan menulis kode.

| Janji | Mekanisme di Nusa |
|---|---|
| Aman memori | Tanpa pointer mentah & tanpa aritmetika pointer. Kepemilikan (ownership) + peminjaman (borrowing) diperiksa saat kompilasi; tidak ada *use-after-free*, *double-free*, atau data race. Tanpa *garbage collector*. |
| Aman tipe | Tipe statis dengan inferensi lokal, bilangan bertanda/tak bertanda berukuran tetap, `teks` (&str) terpisah dari `Daftar<huruf>`, dan konversi wajib eksplisit (`sebagai`). |
| Aman runtime | Setiap operasi berisiko (indeks larik, luber bilangan, bagi nol) punya **penjaga** (`jaga`) yang menyala saat jalan — kecuali terbukti aman oleh pembuktian otomatis. |
| Cepat | Kode mesin native lewat LLVM (`llc -O2`). Setara C pada tolok ukur (lihat §6). |
| Kecil | Runtime hanya ±90 baris C, biner "halo dunia" ±17 KB, kompiler sendiri ±0,9 MB, tanpa dependensi Rust pihak ketiga. |
| Dapat diverifikasi | Kontrak `perlu`/`jamin`, invarian loop, dan `tegas` diterjemahkan menjadi **kondisi verifikasi SMT-LIB2** dan dibuktikan **Z3**. Dengan `--bebas-jaga`, biner hanya dibangun bila *semua* tugas terbukti. |
| Dua cara eksekusi | `nusa jalan` **menafsirkan** sumber langsung (tanpa biner, untuk iterasi cepat); `nusa bangun` **mengompilasi** ke mesin lewat LLVM (untuk rilis). Keduanya berbagi semantik, penjaga runtime, dan pesan galat yang sama — diuji setara (§2). |
| Ergonomis | Kata kunci bahasa Indonesia, tanda kurung opsional, `ubah`/`tetap`, `selama`/`untuk`/`jika`/`selain`, kontrak di atas fungsi, dan pesan galat berbahasa Indonesia dengan kode stabil. |

> Status: **v0.1** — cukup lengkap untuk program nyata berskala kecil
> (lihat `contoh/`), dengan batasan yang didaftar jujur di §9.

---

## 1. Pasang dan pakai

```bash
# Butuh: rustc/cargo, llc (LLVM 19) atau clang, cc, dan z3 (untuk `bukti`)
cargo build --release

./target/release/nusa jalan contoh/halo.ns     # langsung ditafsirkan
./target/release/nusa bangun contoh/halo.ns -o halo && ./halo   # dikompilasi
```

```
Halo, Nusantara!
```

Perintah:

```bash
nusa jalan    berkas.ns             # EKSEKUSI 1: penafsir (tanpa biner)
nusa bangun   berkas.ns [-o keluar] # EKSEKUSI 2: kompilasi ke biner native
nusa jalankan berkas.ns             # kompilasi lalu jalankan binernya
nusa periksa  berkas.ns             # tipe + kepemilikan + peminjaman (cepat)
nusa ir       berkas.ns             # cetak LLVM IR
nusa bukti    berkas.ns             # buktikan kontrak dengan Z3
nusa --smt    berkas.ns             # nama lain untuk `bukti`
```

Opsi: `--opt 0..3` (bawaan 2), `--simpan-ir`, `--bebas-jaga`
(buang penjaga runtime saat kompilasi; **ditolak** bila ada tugas yang belum
terbukti), dan `--boleh-takaman` (izin eksplisit untuk blok `takaman`).
Ekstensi berkas sumber Nusa adalah **`.ns`**.

Pipeline internal:

```
berkas.ns
   │  lex.rs      → token
   │  parse.rs    → AST (Pratt parser)
   │  sema.rs     → tipe, kepemilikan, peminjaman
   │  smt.rs      → VC (SMT-LIB2) → z3            [opsional / --bebas-jaga]
   ├── interp.rs  → eksekusi langsung AST          [nusa jalan]
   └── llvm.rs    → LLVM IR tekstual
          │  llc -O2      → berkas objek
          │  cc + runtime → biner ELF             [nusa bangun]
          ▼
        biner
```

---

## 2. Dua cara eksekusi

| | `nusa jalan` (penafsir) | `nusa bangun` (kompiler) |
|---|---|---|
| Keluaran | tidak ada biner | biner ELF native |
| Mulai jalan | langsung (parse → tipe → eksekusi) | parse → tipe → IR → llc → tautan |
| Kecepatan program | ±300× lebih lambat (penafsir pohon) | setara C (LLVM `-O2`) |
| Waktu mulai | ±5 ms, tidak perlu LLVM | ±60 ms termasuk `llc` + `cc` |
| Cocok untuk | belajar, uji cepat, *scripting*, demo | rilis, tolok ukur, sistem terbenam |
| Semantik | **sama**: penjaga batas/luber/bagi-nol, pesan `Nusa: …`, kode keluar | **sama** |
| Kontrak `perlu`/`jamin` | diperiksa & dibuktikan seperti biasa; `tegas` tetap dicek saat jalan | idem, dan penjaga bisa dibuang dengan `--bebas-jaga` |

Contoh, keluaran identik dari kedua mode:

```console
$ nusa jalan contoh/pencarian.ns
cari 10 -> 4
cari 7  -> -1
terurut  -> benar

$ nusa bangun contoh/pencarian.ns -o cari && ./cari
cari 10 -> 4
cari 7  -> -1
terurut  -> benar
```

Jaminan kesetaraan ini diuji otomatis: `uji/jalankan.sh` menjalankan setiap
contoh lewat **kedua** mode dan membandingkan keluarannya dengan berkas
harapan yang sama (`uji/harapan/*.txt`), termasuk kasus penjaga runtime
(`contoh/jaga.ns` menangkap indeks di luar batas di kedua mode) serta
`contoh/padu.ns` yang memakai struk, enumerasi, `huruf`, `nat`, `pecah`,
`Daftar<T>`, dan `teks` sekaligus.

Penafsir sengaja **tidak** melonggarkan keamanan: ia selalu memeriksa batas
indeks, luber, dan pembagian nol walau `--bebas-jaga` diberikan — pembuangan
penjaga hanya berlaku pada biner yang sudah terbukti.

---

## 3. Tur bahasa

### 3.1 Halo dunia

```
fungsi utama() -> bil {
    cetak("Halo, Nusantara!")
    kembali 0
}
```

Tanpa titik koma: akhir baris sudah memisahkan pernyataan (seperti Go).

### 3.2 Kata kunci

| Nusa | Arti |
|---|---|
| `fungsi` | function |
| `struk` | struct |
| `enumerasi` | enum |
| `ubah` / `tetap` | var (mutable) / let (immutable) |
| `jika` / `selain` | if / else |
| `selama` | while |
| `untuk … dalam …` | for-in / for-range |
| `kembali` | return |
| `henti` / `lanjut` | break / continue |
| `tegas` | assert |
| `perlu` / `jamin` | precondition / postcondition |
| `invarian` | loop invariant |
| `sebagai` | cast |
| `impor` | import modul |
| `jenis` | alias tipe |
| `pilihan` / `cocok` / `kasus` | enum berlabel / match / case |
| `takaman` | blok tidak aman (perlu `--boleh-takaman`) |
| `benar` / `salah` | true / false |
| `dan` / `atau` / `bukan` | and / or / not |

### 3.3 Tipe

| Tipe | Isi | Ukuran |
|---|---|---|
| `bil` | bilangan bulat bertanda | 64-bit |
| `bil32`, `bil8` | bertanda lebih kecil | 32 / 8-bit |
| `nat`, `nat8` | tak bertanda | 64 / 8-bit |
| `pecah` | pecahan IEEE-754 | 64-bit |
| `log` | boolean | 1-bit |
| `huruf` | satu byte (`'a'`) | 8-bit |
| `teks` | irisan byte tak berubah (`"..."`) | 16 byte |
| `[T; N]` | larik tetap | N × sizeof(T) |
| `[T]` | irisan (slice) | 16 byte |
| `Daftar<T>` | vektor dinamis di heap | 24 byte |
| `&T`, `&ubah T` | pinjaman | 8 byte |
| `struk N { … }` | rekaman bernama | sesuai medan |
| `enumerasi E { … }` | varian bernomor | 32-bit |

Literal bulat menyesuaikan tipe target bila muat (`tetap x: bil8 = 7`),
tetapi tidak pernah menyempit diam-diam dari variabel — pakai `sebagai`.

### 3.4 Variabel, mutabilitas, dan cakupan

```
ubah hitungan: bil = 0;      // boleh diubah
tetap nama: teks = "Nusa";   // tidak boleh diubah (diperiksa kompilator)
```

Inferensi tipe lokal: `ubah x = 5` (tipe `bil`). Deklarasi tanpa tipe untuk
`daftar()` wajib anotasi.

### 3.5 Kendali alur

```
jika x > 0 {
    cetak("positif")
} selain jika x < 0 {
    cetak("negatif")
} selain {
    cetak("nol")
}

untuk i dalam 0..10 { … }        // 0..9
untuk i dalam 0..=10 { … }       // 0..=10
untuk x dalam larik { … }        // setiap elemen

selama i < n {
    i = i + 1
}
```

### 3.6 Fungsi dan kontrak

```
fungsi cari(a: [bil], kunci: bil) -> bil
    perlu panjang(a) > 0
    perlu panjang(a) <= 1000000
    jamin hasil >= -1
    jamin hasil < panjang(a)
{
    …
}
```

`hasil` menamai nilai kembalian di dalam `jamin`. Parameter `&ubah` adalah
pinjaman eksklusif, `&T` pinjaman bersama.

### 3.7 Struk dan enumerasi

```
struk Titik { x: bil, y: bil }
enumerasi Arah { Utara, Selatan, Timur, Barat }

ubah t: Titik = Titik { x: 0, y: 1 };
cetak(t.x, " ", Arah::Timur)     // mencetak nama varian: "Timur"

pilihan Status { Aktif, Nonaktif }

cocok s {
    kasus Aktif    { cetak("aktif") }
    kasus Nonaktif { cetak("nonaktif") }
}
```

`cocok` harus menyebut setiap varian dari `pilihan`/`enumerasi` yang dicocokkan;
varian yang hilang (`E0382`) dan varian asing (`E0380`) ditolak saat periksa.

Blok `takaman { … }` mematikan penjaga runtime untuk isinya (luber, batas
indeks, bagi nol, geser tidak sah). Karena itu blok ini **tidak** termasuk
jaminan aman-memori bawaan: program ditolak dengan `E0385` kecuali kompilasi
dijalankan dengan `--boleh-takaman`, dan setiap pemakaian sebaiknya diberi
alasan serta dikurung seluas mungkin.

### 3.8 Larik, irisan, dan `Daftar<T>`

```
tetap tabel: [bil; 4] = [1, 2, 3, 4];
cetak(tabel[2], panjang(tabel))

fungsi jumlah(a: [bil]) -> bil {
    ubah s: bil = 0;
    untuk x dalam a { s = s + x }
    kembali s
}

ubah d: Daftar<bil> = daftar();
tambah(&ubah d, 42);            // meminjam secara eksklusif
ubah e: Daftar<bil> = daftar();
e.tambah(7); e.tambah(8);       // bentuk metode (penerima harus `ubah`)
cetak(ambil(d, 0), panjang(d), e.panjang())
```

### 3.9 Teks

```
tetap s: teks = "nusantara";
cetak(panjang(s))        // 9
cetak(s[0] == 'n')       // benar
cetak(s == "nusantara")  // perbandingan isi, bukan alamat
cetak(s < "nusantaraz")  // urutan leksikografis
```

`teks` adalah irisan byte **tak berubah** kepunyaan orang lain: tidak bisa
diubah, tidak dialokasikan, tidak perlu dibebaskan.

### 3.10 Metode, alias, dan modul

Bentuk metode hanya mengubah gula sintaks: `d.tambah(x)` setara
`tambah(&ubah d, x)`, dan `d.panjang()` setara `panjang(d)`. Penerima yang
tidak dideklarasikan `ubah` ditolak dengan `E0383`.

Alias tipe dideklarasikan dengan `jenis Nama = Tipe;`; alias hanya nama lain
untuk tipe yang sudah dikenal dan tidak membuat tipe nominal baru. Modul lain
dimuat dengan `impor "berkas"` (tanpa ekstensi) secara rekursif; setiap berkas
dimuat sekali, dan deklarasi yang bentrok dilaporkan sebagai galat alih-alih
digabung diam-diam.

### 3.11 Cetak

`cetak(a, b, c)` mencetak semua argumen lalu satu baris baru; `tulis(...)`
tanpa baris baru. Nilai `log` dicetak sebagai `benar`/`salah`, `enumerasi`
sebagai nama variannya.

---

## 4. Keamanan memori

Nusa tidak punya pointer mentah, `malloc`, atau aritmetika pointer di permukaan
bahasa. Yang ada hanya nilai, pinjaman, dan aturan pemilikannya:

1. **Kepemilikan tunggal.** Nilai yang memiliki memori heap (`Daftar<T>`)
   hanya punya satu pemilik. Menyerahkannya ke fungsi lain berarti
   **memindahkan** (move) — pemilik lama tidak boleh dipakai lagi.
2. **Peminjaman.** `&ubah x` memberi akses eksklusif; `&x` memberi akses baca.
   Tidak boleh ada dua peminjaman eksklusif, atau eksklusif bersamaan dengan
   bersama. Pinjaman tidak boleh keluar dari fungsi (anti referensi menggantung).
3. **Pemeriksaan batas.** Setiap `a[i]` punya kewajiban `0 <= i < panjang(a)`.
   Bila terbukti oleh Z3, penjaga dibuang. Bila tidak, penjaga tetap ada dan
   menghentikan program dengan pesan jelas (bukan *undefined behaviour*).
4. **Pembebasan otomatis.** Pemilik `Daftar<T>` membebaskan memorinya di akhir
   cakupan — tepat sekali, tanpa *leak* dan tanpa `double free`. Pemindahan
   mengosongkan nilai sumber, sehingga pembebasan ganda mustahil; `Daftar`
   di dalam `struk` ikut dibebaskan secara rekursif. Kepemilikan bersarang
   (`Daftar<Daftar<T>>`, larik berisi `Daftar`) ditolak saat kompilasi (E0203).

Semua pelanggaran ditolak saat kompilasi dengan kode galat stabil:

```console
$ nusa periksa uji/tolak/pindah.ns
galat[E0320]: `d` sudah dipindahkan (move); nilai tidak boleh dipakai lagi
  --> uji/tolak/pindah.ns:8:19
```

| Kode | Arti |
|---|---|
| `E0306` | menulis ke variabel `tetap` |
| `E0320` | memakai nilai yang sudah dipindahkan |
| `E0327` | operasi pada tipe yang tidak cocok |
| `E0333` | peminjaman eksklusif ganda |
| `E0358` | `daftar()` tanpa anotasi tipe |
| `E0371` | mengembalikan pinjaman (referensi menggantung) |
| `E0211` | ada jalur yang tidak mengembalikan nilai |

### 4.1 Contoh: peminjaman eksklusif

```
ubah d: Daftar<bil> = daftar();
tetap p = &ubah d
tambah(&ubah d, 1)      // ditolak: E0333 (p masih meminjam)
```

```
fungsi jumlahkan(d: Daftar<bil>) -> bil { … }   // mengambil alih kepemilikan

ubah d: Daftar<bil> = daftar();
tambah(&ubah d, 1)
cetak(jumlahkan(d))     // d dipindahkan ke fungsi
cetak(panjang(d))       // ditolak: E0320
```

---

## 5. Verifikasi formal

### 5.1 Cara kerja

Setiap fungsi yang tipenya dapat dimodelkan (bilangan bulat, `log`, larik,
irisan) diterjemahkan menjadi **kondisi verifikasi** (VC) dalam SMT-LIB2:

* parameter menjadi simbol dengan batas domain tipe (`i64::MIN ≤ x ≤ i64::MAX`,
  `0 ≤ len ≤ i64::MAX`);
* `perlu` menjadi asumsi;
* setiap operasi berisiko menjadi **kewajiban**: `-2^63 ≤ a+b ≤ 2^63-1`,
  `b ≠ 0`, `0 ≤ i < panjang(a)`;
* `jika`/`selain` memecah jalur (nilai digabung dengan `ite`);
* loop di-*havoc*, lalu **invarian** dipakai sebagai asumsi masuk dan kewajiban
  keluar;
* `kembali` memeriksa `jamin` (dengan `hasil` = nilai kembali);
* pemanggilan fungsi memeriksa `perlu` si pemanggil dan **mengasumsikan**
  `jamin`-nya — jadi verifikasi bersifat modular dan rekursif.

Untuk setiap kewajiban, Nusa menanyakan `(assert (not (=> (and asumsi…) tugas)))`
ke Z3:

* `unsat` → **terbukti** (tidak ada jalan melanggar),
* `sat` → **tak terbukti**, disertai contoh penyangkal dari model Z3,
* `timeout` → **tidak diketahui**.

Ekspresi spesifikasi (`perlu`, `jamin`, `invarian`) diperlakukan sebagai
**aritmetika matematis** (tanpa pembungkusan), sehingga kontrak tidak pernah
"meluap" saat dibuktikan.

### 5.2 Apa yang dibuktikan pada contoh `contoh/pencarian.ns`

```console
$ nusa bukti contoh/pencarian.ns
verifikasi formal Nusa (SMT-LIB2 + Z3)
  [ok] cari
          terbukti  baris 11   tidak luber pada `-`
          terbukti  baris 15   tidak luber pada `+`
          terbukti  baris 12   invarian berlaku saat masuk loop
          …
          terbukti  baris 22   indeks dalam batas (0 <= i < a_1_len)
  [ok] terurut
  …
ringkasan: 34 tugas, 34 terbukti, 0 tak terbukti, 0 tidak diketahui, 0 fungsi dilewati
```

Artinya: untuk **semua** panjang larik `1 ≤ len ≤ 10^6` dan **semua** isi
larik, pencarian biner tidak pernah membaca di luar batas, tidak pernah
meluap, dan memenuhi `-1 ≤ hasil < panjang(a)`.

Bila kewajiban tidak terbukti — misalnya karena kontrak kurang kuat —
laporannya jujur dan menyertakan contoh penyangkal:

```console
$ nusa bukti contoh/jaga.ns
  [!!] ambil_bebas
          TAK TERBUKTI baris 3    indeks dalam batas (0 <= i < a_1_len)
                    contoh penyangkal: a_1_len = 0  i_1 = -1
```

### 5.3 `--bebas-jaga`: menghapus penjaga hanya bila aman

```console
$ nusa bangun contoh/jaga.ns -o jaga --bebas-jaga
nusa: `--bebas-jaga` ditolak: masih ada tugas yang belum terbukti
```

Ini model **gagal-aman**: pemeriksaan runtime hanya boleh dibuang jika
pembuktian formal berhasil untuk seluruh program. Untuk kode yang terbukti,
biayanya nol.

---

## 6. Penjaga runtime

| Situasi | Tanpa bukti | Dengan bukti |
|---|---|---|
| `a[i]` di luar batas | `Nusa: indeks di luar batas pada baris 3`, keluar kode 1 | penjaga dihapus |
| `a + b` luber | `Nusa: luber bilangan (overflow)` | penjaga dihapus |
| `a / 0` | `Nusa: pembagian dengan nol` | penjaga dihapus |
| `tegas salah` | `Nusa: tegas gagal pada baris N: <pesan>` | — |
| `gagal("…")` | `Nusa: <pesan>` | — |

Penjaga memakai `llvm.sadd.with.overflow` / `icmp`, bukan trap perangkat
keras, sehingga pesan galat konsisten dan portabel.

---

## 7. Kinerja dan ukuran

Tolok ukur `bench/prima.ns` (hitung bilangan prima < 200.000, percobaan
bagi) di mesin yang sama:

| Mode | Waktu | Keterangan |
|---|---|---|
| **Nusa dikompilasi** (`nusa bangun`, `llc -O2`) | **16,4 ms** | penjaga batas & luber tetap aktif di `prima(k)` |
| C (`cc -O2`) | 16,1 ms | tanpa pemeriksaan apa pun |
| **Nusa ditafsirkan** (`nusa jalan`) | 5,5 s | penafsir pohon, tanpa optimasi |
| Python 3 | 415,7 ms | — |

Kode Nusa yang dikompilasi berjalan **setara C** — keduanya turun ke mesin
lewat LLVM — dan sekitar 25× lebih cepat daripada Python untuk beban ini.
Mode penafsir ±300× lebih lambat daripada kode native: wajar untuk penafsir
pohon, dan itulah harga yang dibayar untuk *tanpa* langkah kompilasi.

Ukuran:

| Artefak | Ukuran |
|---|---|
| kompiler+penafsir `nusa` (release, tanpa dependensi luar) | 1,0 MB |
| biner "halo dunia" | 17,0 KB |
| biner tolok ukur prima | 17,2 KB |
| runtime C (sumber) | ±90 baris |
| total sumber kompiler + penafsir | ±6.600 baris Rust |

Waktu kompilasi `contoh/pencarian.ns` (parse → tipe → IR → llc → tautan):
±64 ms. Pembuktian 34 kewajiban SMT: ±0,5 detik.

Memori: biner Nusa tidak membawa *runtime* bahasa (tanpa GC, tanpa mesin
virtual); penggunaan memori hanya milik program itu sendiri + libc. Mode
`nusa jalan` memakai memori proses kompiler (beberapa MB) dan tidak
menghasilkan berkas apa pun.
`Daftar<T>` tumbuh 4 → 8 → 16 … sehingga alokasi ulang jarang terjadi.

---

## 8. Arsitektur

| Berkas | Isi | Baris |
|---|---|---|
| `src/lex.rs` | lexer, kata kunci Indonesia, `//` dan `/* */` | ±330 |
| `src/parse.rs` | parser Pratt + pernyataan, tanpa titik koma | ±1.000 |
| `src/sema.rs` | tipe, mutabilitas, kepemilikan, peminjaman, kontrak | ±1.500 |
| `src/llvm.rs` | pembangkit LLVM IR tekstual (SSA via alloca), penjaga | ±1.500 |
| `src/interp.rs` | penafsir pohon: nilai, wadah, penjaga, bawaan, cetak | ±950 |
| `src/smt.rs` | pembangkit VC + SMT-LIB2 + panggilan Z3 | ±1.000 |
| `src/diag.rs` | diagnostik berwarna dengan kode stabil | ±90 |
| `src/main.rs` | CLI: `jalan`/`bangun`/`jalankan`/`periksa`/`ir`/`bukti` | ±360 |
| `runtime/nusa_rt.c` | pencetakan, pelanggaran, vektor | ±90 |

IR yang dihasilkan memakai satu `alloca` per variabel (gaya `-O0`) dan
menyerahkan seluruh optimasi (termasuk *mem2reg*) ke LLVM — sederhana untuk
dibangkitkan, tetap cepat karena `llc -O2`.

---

## 9. Uji

```bash
cargo test                 # 45 uji regresi Rust (penafsir + biner bila LLVM ada)
./uji/jalankan.sh          # uji ujung-ke-ujung + kesetaraan dua mesin
```

```
test result: ok. 45 passed; 0 failed
ringkasan uji: 48 lulus, 0 gagal
```

Cakupan: 8 program contoh (periksa + bangun + tafsir), **kesetaraan penafsir
dengan biner** pada 7 contoh dan satu program unit (bilangan bertipe sempit,
`sebagai`, `lanjut`/`henti`, teks, larik), verifikasi formal (semua terbukti
vs. jujur tak terbukti), penjaga runtime di kedua mode, penolakan
`--bebas-jaga`, dan 8 program tidak aman yang **wajib** ditolak dengan kode
galat tertentu — termasuk saat dijalankan lewat penafsir. Uji Rust menambahkan
regresi untuk literal `i64::MIN`, `MIN / -1` dan `MIN % -1`, penulisan elemen
larik tetap, lingkup blok/iterasi, peminjaman wadah `untuk ... dalam`, impor
modul dengan posisi sumber sama, penolakan `--bebas-jaga` saat ada fungsi
dilewati, serta keluarnya loop yang tidak vakum di verifikator, konversi
`pecah`↔bulat (termasuk NaN/luar rentang), geser di luar lebar,
`takaman`/`--boleh-takaman`, kelengkapan `cocok`, penerima metode `tetap`,
batas kedalaman parser, batas rekursi penafsir, impor modul/duplikasi (di
kedua mesin), serta pinjaman dan pemindahan medan struk.

---

## 10. Batasan v0.1 dan peta jalan

Yang **sudah** ada: tipe dasar + larik/irisan/`Daftar<T>`, struk, enumerasi
tanpa payload, `pilihan` + `cocok` dengan pemeriksaan kelengkapan, alias
`jenis`, `impor` modul dengan deteksi duplikasi, kontrol alur lengkap, kontrak
+ invarian + `tegas`, verifikasi SMT untuk tipe bulat dan larik, penjaga
runtime, `--bebas-jaga`, blok `takaman` yang wajib diaktifkan eksplisit, pesan
galat berbahasa Indonesia, **dua mode eksekusi (penafsir + kompiler)**, dan
CLI lengkap.

Yang **belum** (dan tidak diklaim ada):

* pencocokan pola berpayload dan enumerasi berpayload (`E0101` dari parser);
* alias `cacah32` saat ini dipetakan ke `nat` (u64), bukan lebar u32; tipe
  nominal berlebar tetap untuk alias belum ada;
* konkatenasi/pemotongan `teks` dan `Daftar<teks>` (butuh model kepemilikan
  string yang lebih kaya);
* fungsi generik, closure, dan tipe hasil `impl`/trait;
* `Daftar<T>` untuk `T` non-skalar (kepemilikan bersarang ditolak, E0203);
* larik yang memuat `Daftar<T>`;
* verifikasi untuk `pecah`, `teks`, `Daftar<T>`, dan fungsi yang memakai
  `henti`/`lanjut` (dilaporkan sebagai "dilewati"/"belum dimodelkan", bukan
  diklaim terbukti);
* pemilihan simbol per-modul dan impor selektif belum ada; semua deklarasi
  impor digabung ke satu ruang nama, dan benturan nama dilaporkan sebagai
  galat; impor melingkar diabaikan lewat penanda berkas yang sudah dimuat;
* `&ubah T` hanya untuk meneruskan ke bawaan seperti `tambah` — belum ada
  penulisan langsung lewat referensi (`*p = ...`);
* induksi/lemma untuk spesifikasi fungsional (mis. `hasil == n!`), sehingga
  sebagian kontrak perkalian rekursif tetap "tak terbukti" — penjaga runtime
  yang menutupnya;
* pada penafsir: `--bebas-jaga` diabaikan (penjaga tetap menyala demi
  keamanan), dan pencetakan `pecah` memakai pendekatan `%g` yang bisa
  berbeda pada bentuk bilangan sangat besar/kecil;
* penafsir dan kompiler sama-sama belum mendukung closure/generik, jadi
  seluruh bahasa yang dijelaskan di README ini berperilaku sama di kedua mode.

Peta jalan: pola `pilih` → `teks` yang memiliki buffer → modul & `impor` →
REPL interaktif di atas penafsir →
generik → lemma/induksi di `smt.rs`.

---

## 11. Lisensi

Apache-2.0 (lihat `LICENSE` di akar repositori).
