//! Diagnostik kesalahan berbahasa Indonesia.

use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub mulai: usize,
    pub akhir: usize,
}

impl Span {
    pub fn baru(mulai: usize, akhir: usize) -> Self {
        Span { mulai, akhir }
    }

    pub fn gabung(a: Span, b: Span) -> Self {
        Span {
            mulai: a.mulai.min(b.mulai),
            akhir: a.akhir.max(b.akhir),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub kode: &'static str,
    pub pesan: String,
    pub span: Option<Span>,
    pub catatan: Vec<String>,
}

impl Diag {
    pub fn baru(kode: &'static str, pesan: impl Into<String>) -> Self {
        Diag {
            kode,
            pesan: pesan.into(),
            span: None,
            catatan: Vec::new(),
        }
    }

    pub fn di(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn di_opt(mut self, span: Option<Span>) -> Self {
        if span.is_some() {
            self.span = span;
        }
        self
    }

    pub fn catat(mut self, pesan: impl Into<String>) -> Self {
        self.catatan.push(pesan.into());
        self
    }

    /// Menggambar diagnostik dengan kutipan sumber dan penunjuk `^`.
    pub fn render(&self, sumber: &str, berkas: &str) -> String {
        let mut keluar = String::new();
        let _ = writeln!(keluar, "kesalahan[{}]: {}", self.kode, self.pesan);
        if let Some(span) = self.span {
            let (baris, kolom) = baris_kolom(sumber, span.mulai);
            let _ = writeln!(keluar, "  --> {}:{}:{}", berkas, baris, kolom);
            let awal_baris = sumber[..span.mulai.min(sumber.len())]
                .rfind('\n')
                .map(|p| p + 1)
                .unwrap_or(0);
            let isi = sumber[awal_baris..].split('\n').next().unwrap_or("");
            let tampil = isi.replace('\t', "    ");
            let kolom_tampil: usize = sumber[awal_baris..span.mulai.min(sumber.len())]
                .chars()
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum();
            let akhir_baris = awal_baris + isi.len();
            let akhir = span.akhir.min(akhir_baris).max(span.mulai.min(akhir_baris));
            let panjang: usize = sumber[span.mulai.min(akhir_baris)..akhir]
                .chars()
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum::<usize>()
                .max(1);
            let nomor = baris.to_string();
            let _ = writeln!(keluar, "   {:>width$} |", "", width = nomor.len());
            let _ = writeln!(keluar, "   {} | {}", nomor, tampil);
            let _ = writeln!(
                keluar,
                "   {:>width$} | {}{}",
                "",
                " ".repeat(kolom_tampil),
                "^".repeat(panjang),
                width = nomor.len()
            );
        }
        for c in &self.catatan {
            let _ = writeln!(keluar, "   = catatan: {}", c);
        }
        keluar
    }
}

pub type Hasil<T> = Result<T, Diag>;

/// Mengembalikan (baris, kolom) berbasis 1 untuk sebuah offset byte.
pub fn baris_kolom(sumber: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(sumber.len());
    let mut baris = 1;
    let mut awal = 0;
    for (i, c) in sumber.char_indices() {
        if i >= pos {
            break;
        }
        if c == '\n' {
            baris += 1;
            awal = i + 1;
        }
    }
    let kolom = sumber[awal..pos].chars().count() + 1;
    (baris, kolom)
}
