/* Runtime Nusa: pemeriksaan runtime, pencetakan, dan Daftar (vektor). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

void nusa_gagal(int32_t kode, int32_t baris) {
    const char *pesan = "pelanggaran runtime";
    switch (kode) {
    case 1: pesan = "luber bilangan (overflow)"; break;
    case 2: pesan = "indeks di luar batas"; break;
    case 3: pesan = "pembagian dengan nol"; break;
    case 4: pesan = "geser tidak sah"; break;
    default: break;
    }
    fprintf(stderr, "Nusa: %s pada baris %d\n", pesan, (int)baris);
    fflush(stderr);
    exit(1);
}

void nusa_tegas(const char *pesan, int64_t len, int32_t baris) {
    fprintf(stderr, "Nusa: `tegas` gagal pada baris %d: %.*s\n", (int)baris, (int)len, pesan);
    fflush(stderr);
    exit(1);
}

void nusa_panic(const char *pesan, int64_t len) {
    fflush(stdout);
    fputs("Nusa: ", stderr);
    fwrite(pesan, 1, (size_t)len, stderr);
    fputc('\n', stderr);
    exit(1);
}

/* Daftar<T>: { T* data, i64 panjang, i64 kapasitas } */
void nusa_daftar_tambah(void **data, int64_t *len, int64_t *cap, void *elemen, int64_t ukuran) {
    if (*len == *cap) {
        int64_t baru = *cap ? *cap * 2 : 4;
        void *p = realloc(*data, (size_t)(baru * ukuran));
        if (!p) nusa_gagal(4, 0);
        *data = p;
        *cap = baru;
    }
    memcpy((char *)(*data) + (*len) * ukuran, elemen, (size_t)ukuran);
    (*len)++;
}

void nusa_daftar_sisip(void **data, int64_t *len, int64_t *cap, int64_t idx, void *elemen, int64_t ukuran) {
    if (idx < 0 || idx > *len) nusa_gagal(2, 0);
    if (*len == *cap) {
        int64_t baru = *cap ? *cap * 2 : 4;
        void *p = realloc(*data, (size_t)(baru * ukuran));
        if (!p) nusa_gagal(4, 0);
        *data = p;
        *cap = baru;
    }
    memmove((char *)(*data) + (idx + 1) * ukuran, (char *)(*data) + idx * ukuran, (size_t)((*len - idx) * ukuran));
    memcpy((char *)(*data) + idx * ukuran, elemen, (size_t)ukuran);
    (*len)++;
}

void nusa_daftar_bebas(void *p) { free(p); }

int32_t nusa_banding_memori(const void *a, const void *b, int64_t n) {
    int r = memcmp(a, b, (size_t)n);
    return (r > 0) - (r < 0);
}

int32_t nusa_teks_sama(const char *a, int64_t la, const char *b, int64_t lb) {
    return la == lb && (la == 0 || memcmp(a, b, (size_t)la) == 0);
}

int32_t nusa_teks_banding(const char *a, int64_t la, const char *b, int64_t lb) {
    int64_t n = la < lb ? la : lb;
    int r = n ? memcmp(a, b, (size_t)n) : 0;
    if (r != 0) return (r > 0) - (r < 0);
    return (la > lb) - (la < lb);
}

void nusa_baris(void) { putchar('\n'); }

typedef struct { const char *p; int64_t n; } nusa_teks;

void nusa_tulis_varian(int32_t tag, const nusa_teks *tabel, int32_t n) {
    if (tag >= 0 && tag < n) fwrite(tabel[tag].p, 1, (size_t)tabel[tag].n, stdout);
    else printf("<varian %d>", (int)tag);
}

void nusa_cetak_varian(int32_t tag, const nusa_teks *tabel, int32_t n) {
    nusa_tulis_varian(tag, tabel, n);
    putchar('\n');
}

void nusa_cetak_bil(int64_t v) { printf("%lld\n", (long long)v); }
void nusa_tulis_bil(int64_t v) { printf("%lld", (long long)v); }
void nusa_cetak_nat(int64_t v) { printf("%llu\n", (unsigned long long)v); }
void nusa_tulis_nat(int64_t v) { printf("%llu", (unsigned long long)v); }
void nusa_cetak_pecah(double v) { printf("%g\n", v); }
void nusa_tulis_pecah(double v) { printf("%g", v); }
void nusa_cetak_log(int32_t v) { printf("%s\n", v ? "benar" : "salah"); }
void nusa_tulis_log(int32_t v) { printf("%s", v ? "benar" : "salah"); }
void nusa_cetak_huruf(int8_t v) { putchar((unsigned char)v); putchar('\n'); }
void nusa_tulis_huruf(int8_t v) { putchar((unsigned char)v); }
void nusa_cetak_teks(const char *p, int64_t n) { fwrite(p, 1, (size_t)n, stdout); putchar('\n'); }
void nusa_tulis_teks(const char *p, int64_t n) { fwrite(p, 1, (size_t)n, stdout); }
