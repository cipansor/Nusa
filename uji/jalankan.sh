#!/usr/bin/env bash
# Uji ujung-ke-ujung kompiler + penafsir Nusa.
set -u
cd "$(dirname "$0")/.."
NUSA=${NUSA:-./target/release/nusa}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
lulus=0; gagal=0

ok()   { lulus=$((lulus+1)); printf '  \033[32mlulus\033[0m  %s\n' "$1"; }
no()   { gagal=$((gagal+1)); printf '  \033[31mGAGAL\033[0m  %s\n' "$1"; }

harap_sukses() { # nama, perintah...
  local nama=$1; shift
  if "$@" >"$TMP/keluaran" 2>"$TMP/galat"; then ok "$nama"; else
    no "$nama"; sed -n '1,6p' "$TMP/galat" | sed 's/^/        /'
  fi
}

harap_galat() { # nama, kode, perintah...
  local nama=$1 kode=$2; shift 2
  if "$@" >"$TMP/keluaran" 2>"$TMP/galat"; then
    no "$nama (seharusnya ditolak)"
  elif grep -q "\[$kode\]" "$TMP/galat"; then ok "$nama"
  else no "$nama (kode $kode tidak muncul)"; sed -n '1,4p' "$TMP/galat" | sed 's/^/        /'
  fi
}

# keluaran penafsir == keluaran biner == berkas harapan
harap_sama() { # nama-contoh
  local nama=$1
  local harap="uji/harapan/$nama.txt"
  "$NUSA" jalan "contoh/$nama.ns" >"$TMP/tafsir.out" 2>"$TMP/tafsir.err"
  local kode_tafsir=$?
  "$TMP/biner-$nama" >"$TMP/biner.out" 2>&1
  local kode_biner=$?
  if [ $kode_tafsir -ne 0 ] || [ $kode_biner -ne 0 ]; then
    no "$nama: proses keluar bukan 0"; sed -n '1,4p' "$TMP/tafsir.err" | sed 's/^/        /'; return
  fi
  if ! diff -q "$harap" "$TMP/tafsir.out" >/dev/null; then
    no "$nama: penafsir ≠ harapan"; diff "$harap" "$TMP/tafsir.out" | sed -n '1,8p' | sed 's/^/        /'; return
  fi
  if ! diff -q "$harap" "$TMP/biner.out" >/dev/null; then
    no "$nama: biner ≠ harapan"; diff "$harap" "$TMP/biner.out" | sed -n '1,8p' | sed 's/^/        /'; return
  fi
  ok "$nama: penafsir == biner == harapan"
}

echo "== periksa, bangun, dan tafsirkan semua contoh =="
for f in contoh/*.ns; do
  d=$(basename "$f" .ns)
  harap_sukses "periksa $(basename "$f")" "$NUSA" periksa "$f"
  harap_sukses "bangun  $(basename "$f")" "$NUSA" bangun "$f" -o "$TMP/biner-$d"
  harap_sukses "periksa+tafsir $(basename "$f")" sh -c "'$NUSA' jalan '$f' >/dev/null 2>&1 || [ \$? -eq 1 ]"
done

echo "== kesetaraan penafsir dengan hasil kompilasi =="
for d in halo teks daftar struk pencarian faktorial padu; do
  harap_sama "$d"
done

echo "== unit kecil: bilangan, tipe sempit, teks, kontrol =="
cat > "$TMP/unit.ns" <<'EOF'
fungsi jumlah(a: [bil]) -> bil {
    ubah s: bil = 0;
    untuk x dalam a { s = s + x }
    kembali s
}

fungsi utama() -> bil {
    tetap a: [bil; 4] = [1, 2, 3, 4];
    cetak("jumlah   = ", jumlah(a))
    cetak("panjang  = ", panjang(a))
    ubah b: bil8 = 100;
    b = b + 20
    cetak("bil8     = ", b)
    ubah u: nat8 = 250;
    u = u + 5
    cetak("nat8     = ", u)
    ubah i: bil = 0;
    ubah n: bil = 0;
    selama i < 5 {
        i = i + 1
        jika i % 2 == 0 { lanjut }
        n = n + i
    }
    cetak("ganjil   = ", n)
    untuk i dalam 1..=3 {
        jika i == 2 { henti }
        cetak("loop     = ", i)
    }
    cetak("bagi     = ", 7 / 2, " ", 7 % 2, " ", -7 / 2, " ", -7 % 2)
    cetak("sebagai  = ", 100 sebagai bil8)
    kembali 0
}
EOF
cat > "$TMP/unit.harap" <<'EOF'
jumlah   = 10
panjang  = 4
bil8     = 120
nat8     = 255
ganjil   = 9
loop     = 1
bagi     = 3 1 -3 -1
sebagai  = 100
EOF
if "$NUSA" jalan "$TMP/unit.ns" >"$TMP/unit.tafsir" 2>/dev/null; then
  if diff -q "$TMP/unit.harap" "$TMP/unit.tafsir" >/dev/null; then ok "unit: penafsir"
  else no "unit: penafsir"; diff "$TMP/unit.harap" "$TMP/unit.tafsir" | sed 's/^/        /'; fi
else no "unit: penafsir gagal jalan"; fi
if "$NUSA" bangun "$TMP/unit.ns" -o "$TMP/unit" >/dev/null 2>&1 && "$TMP/unit" >"$TMP/unit.biner" 2>&1; then
  if diff -q "$TMP/unit.harap" "$TMP/unit.biner" >/dev/null; then ok "unit: biner"
  else no "unit: biner"; diff "$TMP/unit.harap" "$TMP/unit.biner" | sed 's/^/        /'; fi
else no "unit: biner gagal"; fi

# penyempitan di luar jangkauan: penafsir dan biner harus sama-sama menangkap
cat > "$TMP/sempit.ns" <<'EOF'
fungsi utama() -> bil {
    cetak(300 sebagai bil8)
    kembali 0
}
EOF
"$NUSA" jalan "$TMP/sempit.ns" >"$TMP/s1" 2>&1
k1=$?
"$NUSA" bangun "$TMP/sempit.ns" -o "$TMP/sempit" >/dev/null 2>&1 && "$TMP/sempit" >"$TMP/s2" 2>&1
k2=$?
if [ $k1 -eq 1 ] && [ $k2 -eq 1 ] && grep -q "luber bilangan (overflow)" "$TMP/s1" && grep -q "luber bilangan (overflow)" "$TMP/s2"; then
  ok "penyempitan di luar jangkauan: penafsir == biner (keduanya menangkap)"
else
  no "penyempitan di luar jangkauan"; sed -n '1,3p' "$TMP/s1" "$TMP/s2" | sed 's/^/        /'
fi

echo "== verifikasi formal =="
harap_sukses "bukti pencarian.ns (semua tugas terbukti)" "$NUSA" bukti contoh/pencarian.ns
if "$NUSA" bukti contoh/faktorial.ns >"$TMP/b" 2>&1; then
  no "bukti faktorial.ns seharusnya melaporkan tugas tak terbukti"
else
  if grep -q "1 tak terbukti" "$TMP/b"; then ok "bukti faktorial.ns jujur (1 tak terbukti)"; else no "bukti faktorial.ns"; fi
fi

echo "== penjaga runtime (penafsir dan biner) =="
"$NUSA" jalan contoh/jaga.ns >"$TMP/j.tafsir" 2>&1
if [ $? -ne 0 ] && grep -q "indeks di luar batas pada baris 3" "$TMP/j.tafsir"; then
  ok "penafsir: indeks di luar batas tertangkap"
else
  no "penafsir: penjaga batas"; sed -n '1,4p' "$TMP/j.tafsir" | sed 's/^/        /'
fi
"$TMP/biner-jaga" >"$TMP/j.biner" 2>&1
if [ $? -ne 0 ] && grep -q "indeks di luar batas pada baris 3" "$TMP/j.biner"; then
  ok "biner: indeks di luar batas tertangkap"
else
  no "biner: penjaga batas"; sed -n '1,4p' "$TMP/j.biner" | sed 's/^/        /'
fi
if "$NUSA" bangun contoh/jaga.ns -o "$TMP/j2" --bebas-jaga >"$TMP/bj" 2>&1; then
  no "bebas-jaga seharusnya menolak"
elif grep -q "belum terbukti" "$TMP/bj"; then ok "bebas-jaga menolak tugas tak terbukti"
else no "bebas-jaga: pesan salah"; fi

echo "== penolakan program tidak aman =="
harap_galat "pakai setelah pindah"   E0320 "$NUSA" periksa uji/tolak/pindah.ns
harap_galat "pinjaman ganda"         E0333 "$NUSA" periksa uji/tolak/pinjam.ns
harap_galat "ubah variabel tetap"    E0306 "$NUSA" periksa uji/tolak/tetap.ns
harap_galat "tipe tidak cocok"       E0327 "$NUSA" periksa uji/tolak/tipe.ns
harap_galat "pinjaman keluar fungsi" E0371 "$NUSA" periksa uji/tolak/pinjaman_keluar.ns
harap_galat "jalur tanpa kembali"    E0211 "$NUSA" periksa uji/tolak/tanpa_kembali.ns
harap_galat "daftar tanpa tipe"      E0358 "$NUSA" periksa uji/tolak/kosong.ns
harap_galat "Daftar bersarang"       E0203 "$NUSA" periksa uji/tolak/bersarang.ns
harap_galat "penafsir pun menolak"   E0320 "$NUSA" jalan uji/tolak/pindah.ns

echo
echo "ringkasan uji: $lulus lulus, $gagal gagal"
[ "$gagal" -eq 0 ]
