//! Diagnostik: pesan galat berbahasa Indonesia dengan penunjuk lokasi.

use crate::ast::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Tingkat {
    Galat,
    Peringatan,
    Catatan,
}

#[derive(Clone, Debug)]
pub struct Diagnostik {
    pub tingkat: Tingkat,
    pub kode: String,
    pub pesan: String,
    pub berkas: String,
    pub span: Span,
    pub saran: Option<String>,
}

impl Diagnostik {
    pub fn galat(berkas: &str, span: Span, pesan: String, kode: &str) -> Self {
        Diagnostik {
            tingkat: Tingkat::Galat,
            kode: kode.to_string(),
            pesan,
            berkas: berkas.to_string(),
            span,
            saran: None,
        }
    }

    #[allow(dead_code)]
    pub fn peringatan(berkas: &str, span: Span, pesan: String, kode: &str) -> Self {
        Diagnostik {
            tingkat: Tingkat::Peringatan,
            kode: kode.to_string(),
            pesan,
            berkas: berkas.to_string(),
            span,
            saran: None,
        }
    }

    pub fn saran(mut self, s: impl Into<String>) -> Self {
        self.saran = Some(s.into());
        self
    }

    pub fn cetak(&self, sumber: &str, berwarna: bool) {
        let (warna, label) = match self.tingkat {
            Tingkat::Galat => ("\x1b[1;31m", "galat"),
            Tingkat::Peringatan => ("\x1b[1;33m", "peringatan"),
            Tingkat::Catatan => ("\x1b[1;36m", "catatan"),
        };
        let (p, q) = if berwarna {
            (warna, "\x1b[0m")
        } else {
            ("", "")
        };
        eprintln!(
            "{}{}[{}]{}: {}",
            p, label, self.kode, q, self.pesan
        );
        let baris_ke = self.span.baris as usize;
        if let Some(baris) = sumber.lines().nth(baris_ke.saturating_sub(1)) {
            let nomor = format!("{}", baris_ke);
            let pad = " ".repeat(nomor.len());
            eprintln!("{} {}--> {}:{}:{}", pad, p, self.berkas, self.span.baris, self.span.kol);
            eprintln!("{} {}|", pad, p);
            eprintln!("{}{} |{} {}", p, nomor, q, baris.replace('\t', " "));
            let kol = self.span.kol as usize;
            let pan = (self.span.panjang as usize).max(1);
            let mut penunjuk = String::new();
            for (idx, ch) in baris.chars().enumerate() {
                if idx + 1 == kol {
                    break;
                }
                penunjuk.push(if ch == '\t' { '\t' } else { ' ' });
            }
            penunjuk.push_str(&"^".repeat(pan.min(baris.len().saturating_sub(kol) + 1).max(1)));
            eprintln!("{} {}| {}{}{}", pad, p, p, penunjuk, q);
        } else {
            eprintln!("  --> {}:{}:{}", self.berkas, self.span.baris, self.span.kol);
        }
        if let Some(s) = &self.saran {
            eprintln!("  {}bantuan{}: {}", p, q, s);
        }
    }
}

pub fn ada_galat(d: &[Diagnostik]) -> bool {
    d.iter().any(|x| x.tingkat == Tingkat::Galat)
}
