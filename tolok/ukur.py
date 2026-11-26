"""Tolok ukur Nusa vs C.

Jalankan:  python3 tolok/ukur.py
Membutuhkan `cc` di PATH dan paket llvmlite/z3 sudah terpasang.
"""
import os
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASA = [sys.executable, os.path.join(ROOT, "bin", "nusa")]

SKALAR_NS = """fungsi utama() {
  ub total: bulat = 0;
  ub i: bulat = 0;
  selama i < 20000000
    invarian 0 <= i
    invarian i <= 20000000
  {
    total += i ^ (total & 3);
    i += 1;
  }
  cetak(total);
}
"""

SKALAR_C = """#include <stdio.h>
int main(void) {
  long total = 0, i = 0;
  while (i < 20000000L) { total += i ^ (total & 3); i += 1; }
  printf("%ld\\n", total);
  return 0;
}
"""

N_DERET = 1024
PUTARAN = 10000


def deret_ns():
    isi = ", ".join(["1"] * N_DERET)
    return f"""fungsi utama() {{
  tp d: deret<bulat, {N_DERET}> = [{isi}];
  ub total: bulat = 0;
  ub putaran: bulat = 0;
  selama putaran < {PUTARAN}
    invarian 0 <= putaran
    invarian putaran <= {PUTARAN}
    invarian 0 <= total
  {{
    untuk i dalam 0..{N_DERET} {{
      total += d[i];
    }}
    putaran += 1;
  }}
  cetak(total);
}}
"""


def deret_c():
    return f"""#include <stdio.h>
int main(void) {{
  static long d[{N_DERET}];
  for (int k = 0; k < {N_DERET}; k++) d[k] = 1;
  long total = 0;
  for (long p = 0; p < {PUTARAN}; p++)
    for (long i = 0; i < {N_DERET}; i++) total += d[i];
  printf("%ld\\n", total);
  return 0;
}}
"""


def build(nama, sumber_ns, sumber_c):
    build_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), ".bangun")
    os.makedirs(build_dir, exist_ok=True)
    ns = os.path.join(build_dir, nama + ".ns")
    c = os.path.join(build_dir, nama + ".c")
    with open(ns, "w", encoding="utf-8") as f:
        f.write(sumber_ns)
    with open(c, "w", encoding="utf-8") as f:
        f.write(sumber_c)
    exe_ns = os.path.join(build_dir, nama + ".nusa")
    exe_c = os.path.join(build_dir, nama + ".cbin")
    subprocess.run(BASA + ["bangun", ns, "-o", exe_ns], check=True,
                   capture_output=True, text=True)
    subprocess.run(["cc", "-O2", c, "-o", exe_c], check=True)
    return exe_ns, exe_c


def ukur(exe, ulang=5):
    waktu = []
    keluaran = None
    for _ in range(ulang):
        mulai = time.perf_counter()
        hasil = subprocess.run([exe], capture_output=True, text=True)
        waktu.append(time.perf_counter() - mulai)
        keluaran = hasil.stdout.strip()
    return statistics.median(waktu), keluaran


def lapor(nama, ns, c):
    exe_ns, exe_c = build(nama, ns, c)
    a, out_a = ukur(exe_ns)
    b, out_b = ukur(exe_c)
    status = "sama" if out_a == out_b else f"BEDA ({out_a} vs {out_b})"
    print(f"{nama:8s} Nusa={a * 1000:7.2f} ms   C -O2={b * 1000:7.2f} ms   "
          f"rasio={a / b:4.2f}x   keluaran={status}")
    print(f"{'':8s} ukuran: Nusa={os.path.getsize(exe_ns)} B, C={os.path.getsize(exe_c)} B")


if __name__ == "__main__":
    lapor("skalar", SKALAR_NS, SKALAR_C)
    lapor("deret", deret_ns(), deret_c())
