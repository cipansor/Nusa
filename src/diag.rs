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
            // Jangan cetak baris sumber yang sangat panjang (berkas jahat):
            // tampilkan jendela di sekitar lokasi galat saja.
            let chars: Vec<char> = baris.chars().collect();
            let kol = self.span.kol as usize;
            let lebar = 120usize;
            let mut mulai = kol.saturating_sub(40);
            if mulai + lebar > chars.len() {
                mulai = chars.len().saturating_sub(lebar);
            }
            let akhir = (mulai + lebar).min(chars.len());
            let awalan = mulai > 0;
            let akhiran = akhir < chars.len();
            let mut tampil = String::new();
            if awalan {
                tampil.push('…');
            }
            tampil.extend(&chars[mulai..akhir]);
            if akhiran {
                tampil.push('…');
            }
            let tampil = tampil.replace('\t', " ");
            eprintln!("{}{} |{} {}", p, nomor, q, tampil);
            let kol_tampil = kol.saturating_sub(mulai) + if awalan { 1 } else { 0 };
            let pan = (self.span.panjang as usize).clamp(1, 40);
            let mut penunjuk = " ".repeat(kol_tampil.saturating_sub(1));
            penunjuk.push_str(&"^".repeat(pan.min(chars.len().saturating_sub(kol) + 1).max(1)));
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
