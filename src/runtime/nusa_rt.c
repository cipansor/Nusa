/* Runtime bahasa Nusa.
 *
 * Model memori:
 *   - Tidak ada pointer yang dapat diakses pengguna; semua nilai heap adalah
 *     pegangan (handle) yang dimiliki secara statis oleh kompiler.
 *   - Setiap objek heap memiliki penghitung referensi (rc) pada offset 8.
 *     rc < 0 berarti objek abadi (literal teks di memori statis).
 *   - Daftar memakai copy-on-write: mutasi pada daftar yang dipakai bersama
 *     menyalin lebih dulu, sehingga semantik nilai tetap terjaga.
 *   - Semua akses daftar diperiksa batasnya; kegagalan memanggil nusa_panik.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PANIK(pesan) nusa_panik((pesan), (int64_t)(sizeof(pesan) - 1))

typedef struct {
    int64_t panjang;
    int64_t rc;
    unsigned char data[];
} NusaTeks;

typedef struct {
    int64_t panjang;
    int64_t rc;
    int64_t kapasitas;
    int64_t jenis; /* 0 = skalar, 1 = teks, 2 = daftar */
    int64_t data[];
} NusaDaftar;

_Static_assert(sizeof(NusaTeks) == 16, "kepala teks harus 16 bita");
_Static_assert(sizeof(NusaDaftar) == 32, "kepala daftar harus 32 bita");

void nusa_panik(const char *pesan, int64_t panjang) {
    fputs("nusa: panik: ", stderr);
    if (panjang > 0) fwrite(pesan, 1, (size_t)panjang, stderr);
    fputc('\n', stderr);
    fflush(stderr);
    exit(134);
}

static void *nusa_alok(int64_t ukuran) {
    if (ukuran < 0) PANIK("ukuran alokasi negatif");
    void *p = malloc((size_t)ukuran);
    if (!p) PANIK("kehabisan memori");
    return p;
}

static inline int64_t *nusa_rc(void *p) {
    return (int64_t *)((unsigned char *)p + 8);
}

void nusa_tahan(void *p) {
    if (!p) return;
    int64_t *rc = nusa_rc(p);
    if (*rc >= 0 && *rc < INT64_MAX) *rc += 1;
}

static void nusa_lepas_elemen(int64_t nilai, int64_t jenis);

void nusa_lepas_teks(NusaTeks *t) {
    if (!t) return;
    if (t->rc < 0) return;
    if (--t->rc == 0) free(t);
}

void nusa_lepas_daftar(NusaDaftar *d) {
    if (!d) return;
    if (d->rc < 0) return;
    if (--d->rc == 0) {
        if (d->jenis != 0) {
            for (int64_t i = 0; i < d->panjang; i++) {
                nusa_lepas_elemen(d->data[i], d->jenis);
            }
        }
        free(d);
    }
}

static void nusa_lepas_elemen(int64_t nilai, int64_t jenis) {
    if (jenis == 1) {
        nusa_lepas_teks((NusaTeks *)(intptr_t)nilai);
    } else if (jenis == 2) {
        nusa_lepas_daftar((NusaDaftar *)(intptr_t)nilai);
    }
}

static void nusa_tahan_elemen(int64_t nilai, int64_t jenis) {
    if (jenis != 0) nusa_tahan((void *)(intptr_t)nilai);
}

NusaTeks *nusa_teks_baru(int64_t panjang) {
    NusaTeks *t = (NusaTeks *)nusa_alok((int64_t)sizeof(NusaTeks) + panjang + 1);
    t->panjang = panjang;
    t->rc = 1;
    t->data[panjang] = 0;
    return t;
}

NusaTeks *nusa_teks_gabung(NusaTeks *a, NusaTeks *b) {
    int64_t la = a ? a->panjang : 0;
    int64_t lb = b ? b->panjang : 0;
    if (la > INT64_MAX - lb) PANIK("panjang teks meluap");
    NusaTeks *t = nusa_teks_baru(la + lb);
    if (la) memcpy(t->data, a->data, (size_t)la);
    if (lb) memcpy(t->data + la, b->data, (size_t)lb);
    return t;
}

int64_t nusa_teks_panjang(NusaTeks *t) {
    return t ? t->panjang : 0;
}

int8_t nusa_teks_sama(NusaTeks *a, NusaTeks *b) {
    int64_t la = a ? a->panjang : 0;
    int64_t lb = b ? b->panjang : 0;
    if (la != lb) return 0;
    if (la == 0) return 1;
    return memcmp(a->data, b->data, (size_t)la) == 0;
}

int64_t nusa_teks_banding(NusaTeks *a, NusaTeks *b) {
    int64_t la = a ? a->panjang : 0;
    int64_t lb = b ? b->panjang : 0;
    int64_t n = la < lb ? la : lb;
    if (n > 0) {
        int r = memcmp(a->data, b->data, (size_t)n);
        if (r != 0) return r < 0 ? -1 : 1;
    }
    if (la == lb) return 0;
    return la < lb ? -1 : 1;
}

NusaDaftar *nusa_daftar_baru(int64_t kapasitas, int64_t jenis) {
    if (kapasitas < 4) kapasitas = 4;
    if (kapasitas > (INT64_MAX - (int64_t)sizeof(NusaDaftar)) / 8)
        PANIK("kapasitas daftar meluap");
    NusaDaftar *d = (NusaDaftar *)nusa_alok((int64_t)sizeof(NusaDaftar) + kapasitas * 8);
    d->panjang = 0;
    d->rc = 1;
    d->kapasitas = kapasitas;
    d->jenis = jenis;
    return d;
}

NusaDaftar *nusa_daftar_salin(NusaDaftar *d) {
    NusaDaftar *salin = nusa_daftar_baru(d->panjang, d->jenis);
    salin->panjang = d->panjang;
    if (d->panjang) {
        memcpy(salin->data, d->data, (size_t)d->panjang * 8);
        if (d->jenis != 0) {
            for (int64_t i = 0; i < d->panjang; i++) nusa_tahan_elemen(salin->data[i], d->jenis);
        }
    }
    return salin;
}

static void nusa_daftar_tumbuh(NusaDaftar **dp) {
    NusaDaftar *d = *dp;
    if (d->panjang < d->kapasitas) return;
    int64_t kapasitas = d->kapasitas < INT64_MAX / 2 ? d->kapasitas * 2 : INT64_MAX;
    if (kapasitas > (INT64_MAX - (int64_t)sizeof(NusaDaftar)) / 8)
        PANIK("kapasitas daftar meluap");
    NusaDaftar *baru = (NusaDaftar *)realloc(d, (size_t)sizeof(NusaDaftar) + (size_t)kapasitas * 8);
    if (!baru) PANIK("kehabisan memori");
    baru->kapasitas = kapasitas;
    *dp = baru;
}

NusaDaftar *nusa_daftar_tambah(NusaDaftar *d, int64_t nilai) {
    if (!d) PANIK("daftar kosong (null)");
    if (d->rc != 1) {
        NusaDaftar *lama = d;
        d = nusa_daftar_salin(lama);
        nusa_lepas_daftar(lama);
    }
    nusa_daftar_tumbuh(&d);
    d->data[d->panjang++] = nilai;
    return d;
}

NusaDaftar *nusa_daftar_isi(NusaDaftar *d, int64_t indeks, int64_t nilai) {
    if (!d) PANIK("daftar kosong (null)");
    if (indeks < 0 || indeks >= d->panjang) {
        char buf[96];
        int n = snprintf(buf, sizeof buf,
                         "indeks di luar batas: indeks=%lld panjang=%lld",
                         (long long)indeks, (long long)d->panjang);
        nusa_panik(buf, n);
    }
    if (d->rc != 1) {
        NusaDaftar *lama = d;
        d = nusa_daftar_salin(lama);
        nusa_lepas_daftar(lama);
    }
    if (d->jenis != 0) {
        nusa_lepas_elemen(d->data[indeks], d->jenis);
    }
    d->data[indeks] = nilai;
    return d;
}

int64_t nusa_daftar_ambil(NusaDaftar *d, int64_t indeks) {
    if (!d) PANIK("daftar kosong (null)");
    if (indeks < 0 || indeks >= d->panjang) {
        char buf[96];
        int n = snprintf(buf, sizeof buf,
                         "indeks di luar batas: indeks=%lld panjang=%lld",
                         (long long)indeks, (long long)d->panjang);
        nusa_panik(buf, n);
    }
    return d->data[indeks];
}

int64_t nusa_daftar_panjang(NusaDaftar *d) {
    return d ? d->panjang : 0;
}

int64_t nusa_bagi(int64_t a, int64_t b) {
    if (b == 0) PANIK("pembagian dengan nol");
    if (a == INT64_MIN && b == -1) PANIK("luapan pembagian");
    int64_t q = a / b;
    int64_t r = a % b;
    if (r < 0) q += (b > 0) ? -1 : 1;
    return q;
}

int64_t nusa_mod(int64_t a, int64_t b) {
    if (b == 0) PANIK("sisa bagi dengan nol");
    if (a == INT64_MIN && b == -1) return 0;
    int64_t r = a % b;
    if (r < 0) r += b > 0 ? b : -b;
    return r;
}

int64_t nusa_mutlak_bil(int64_t a) {
    if (a == INT64_MIN) PANIK("luapan pada mutlak");
    return a < 0 ? -a : a;
}

void nusa_cetak_teks(NusaTeks *t) {
    if (!t || t->panjang == 0) return;
    fwrite(t->data, 1, (size_t)t->panjang, stdout);
}

void nusa_cetak_bil(int64_t v) {
    printf("%lld", (long long)v);
}

void nusa_cetak_des(double v) {
    char buf[64];
    int n = snprintf(buf, sizeof buf, "%.15g", v);
    /* tampilkan bentuk desimal yang jelas untuk bilangan bulat */
    if (n > 0 && !memchr(buf, '.', (size_t)n) && !memchr(buf, 'e', (size_t)n)) {
        snprintf(buf, sizeof buf, "%.1f", v);
        n = (int)strlen(buf);
    }
    fwrite(buf, 1, (size_t)n, stdout);
}

void nusa_cetak_log(int8_t v) {
    if (v) fputs("benar", stdout);
    else fputs("salah", stdout);
}

