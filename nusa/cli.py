"""Antarmuka baris perintah untuk kompiler Nusa."""
import argparse
import os
import subprocess
import sys
import tempfile

from . import __version__
from .interp import GalatTafsir, Panik, Penafsir
from .lexer import GalatLex
from .parser import GalatParse, parse
from .periksa import GalatSemantik, periksa

PIPELINE_OPT = [
    "add_always_inliner_pass",
    "add_sroa_pass",
    "add_instruction_combine_pass",
    "add_simplify_cfg_pass",
    "add_sccp_pass",
    "add_new_gvn_pass",
    "add_aggressive_dce_pass",
    "add_dead_store_elimination_pass",
    "add_loop_rotate_pass",
    "add_loop_strength_reduce_pass",
    "add_loop_unroll_pass",
    "add_reassociate_pass",
    "add_jump_threading_pass",
    "add_instruction_combine_pass",
    "add_aggressive_dce_pass",
]


def _llvm(opt):
    import llvmlite.binding as llvm
    llvm.initialize_native_target()
    llvm.initialize_native_asmprinter()
    tm = llvm.Target.from_default_triple().create_target_machine(opt=opt, reloc="pic")
    return llvm, tm


def optimalkan(ir_text, opt=2):
    llvm, tm = _llvm(opt)
    mod = llvm.parse_assembly(ir_text)
    mod.triple = tm.triple
    mod.verify()
    if opt > 0:
        pb = llvm.create_pass_builder(tm, llvm.PipelineTuningOptions())
        mpm = llvm.create_new_module_pass_manager()
        for nama in PIPELINE_OPT:
            getattr(mpm, nama)()
        mpm.add_verifier()
        mpm.run(mod, pb)
        mod.verify()
    return llvm, tm, mod


def muat_program(path, boleh_takaman):
    with open(path, "r", encoding="utf-8") as f:
        sumber = f.read()
    program = parse(sumber, path)
    konteks = periksa(program, boleh_takaman=boleh_takaman)
    return program, konteks


def _laporkan_galat(e):
    if isinstance(e, GalatSemantik):
        print("galat semantik:", file=sys.stderr)
        for baris, pesan in e.daftar:
            print(f"  baris {baris}: {pesan}", file=sys.stderr)
    else:
        print(f"galat: {e}", file=sys.stderr)


def kompilasi(path, keluaran, opt=2, boleh_takaman=False, strip=False, cc="cc",
              simpan_obj=None, simpan_ir=None):
    program, konteks = muat_program(path, boleh_takaman)
    from .codegen import bangun_ir
    ir_text = bangun_ir(konteks, os.path.basename(path))
    llvm, tm, mod = optimalkan(ir_text, opt)
    if simpan_ir:
        with open(simpan_ir, "w", encoding="utf-8") as f:
            f.write(str(mod) + "\n")
    obj = tm.emit_object(mod)
    tmp = tempfile.mkdtemp(prefix="nusa-")
    obj_path = os.path.join(tmp, "modul.o")
    with open(obj_path, "wb") as f:
        f.write(obj)
    if simpan_obj:
        with open(simpan_obj, "wb") as f:
            f.write(obj)
    perintah = [cc, obj_path, "-o", keluaran]
    if strip:
        perintah.append("-s")
    hasil = subprocess.run(perintah, capture_output=True, text=True)
    if hasil.returncode != 0:
        raise RuntimeError(f"penautan gagal:\n{hasil.stderr}")
    return konteks, len(obj)


def tafsirkan(path, boleh_takaman=False, keluaran=None):
    """Jalankan program dengan penafsir (tanpa LLVM dan tanpa penaut C)."""
    _, konteks = muat_program(path, boleh_takaman)
    for baris, pesan in konteks.peringatan:
        print(f"peringatan baris {baris}: {pesan}", file=sys.stderr)
    penafsir = Penafsir(konteks, keluaran=keluaran)
    try:
        return penafsir.jalankan()
    except Panik as p:
        print(f"nusa: panik: {p.pesan} (baris {p.baris})", file=sys.stderr)
        return 134


def _tambah_opsi(p, optimasi=True):
    p.add_argument("berkas", help="berkas sumber .ns")
    if optimasi:
        p.add_argument("-O", "--opt", type=int, default=2, choices=(0, 1, 2, 3),
                       help="tingkat optimasi LLVM (bawaan 2)")
    p.add_argument("--boleh-takaman", action="store_true",
                   help="izinkan blok 'takaman' (pemeriksaan runtime dimatikan)")
    p.add_argument("--cc", default=os.environ.get("CC", "cc"),
                   help="penaut C yang dipakai (bawaan cc)")


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    ap = argparse.ArgumentParser(
        prog="nusa",
        description="Nusa (.ns): kompiler LLVM dan penafsir untuk bahasa pemrograman Indonesia",
        epilog="Dua cara menjalankan: 'nusa bangun' (kompilasi ke kode mesin) dan "
               "'nusa jalan' (penafsir langsung).")
    ap.add_argument("--versi", action="version", version=f"nusa {__version__}")
    sub = ap.add_subparsers(dest="mode", required=True)

    p_bangun = sub.add_parser("bangun", aliases=["kompilasi"],
                              help="kompilasi menjadi program native (mode kompiler)")
    _tambah_opsi(p_bangun)
    p_bangun.add_argument("-o", "--keluaran", default=None, help="nama berkas keluaran")
    p_bangun.add_argument("-s", "--strip", action="store_true", help="buang simbol")
    p_bangun.add_argument("--obj", default=None, help="simpan berkas objek (.o)")
    p_bangun.add_argument("--ir", default=None, help="simpan IR LLVM hasil optimasi")

    p_jalan = sub.add_parser(
        "jalan", aliases=["jalankan", "interp", "interpretasi"],
        help="jalankan langsung dengan penafsir (interpreter)")
    _tambah_opsi(p_jalan)
    p_jalan.add_argument("--native", action="store_true",
                         help="kompilasi dulu ke kode mesin lalu jalankan (mode kompiler)")
    p_jalan.add_argument("arg", nargs="*", help="argumen program (mode --native)")

    p_ir = sub.add_parser("ir", help="cetak IR LLVM")
    _tambah_opsi(p_ir)

    p_asm = sub.add_parser("asm", help="cetak assembly mesin")
    _tambah_opsi(p_asm)

    p_cek = sub.add_parser("cek", help="periksa sintaks dan tipe saja")
    _tambah_opsi(p_cek)

    p_bukti = sub.add_parser("bukti", help="verifikasi kontrak syarat/jamin dengan Z3")
    _tambah_opsi(p_bukti)
    p_bukti.add_argument("--smt", default=None, help="direktori untuk berkas SMT-LIB")
    p_bukti.add_argument("-v", "--verbose", action="store_true", help="tampilkan semua obligasi")

    args = ap.parse_args(argv)

    try:
        if args.mode == "cek":
            program, konteks = muat_program(args.berkas, args.boleh_takaman)
            n_fn = len(konteks.fungsi)
            n_bt = len(konteks.tipe_bentuk)
            n_pl = len(konteks.tipe_pilihan)
            print(f"ok: {n_fn} fungsi, {n_bt} bentuk, {n_pl} pilihan, 0 galat")
            for baris, pesan in konteks.peringatan:
                print(f"peringatan baris {baris}: {pesan}", file=sys.stderr)
            return 0

        if args.mode == "ir":
            program, konteks = muat_program(args.berkas, args.boleh_takaman)
            from .codegen import bangun_ir
            ir_text = bangun_ir(konteks, os.path.basename(args.berkas))
            _, _, mod = optimalkan(ir_text, args.opt)
            print(str(mod))
            return 0

        if args.mode == "asm":
            program, konteks = muat_program(args.berkas, args.boleh_takaman)
            from .codegen import bangun_ir
            ir_text = bangun_ir(konteks, os.path.basename(args.berkas))
            _, tm, mod = optimalkan(ir_text, args.opt)
            print(tm.emit_assembly(mod))
            return 0

        if args.mode == "bukti":
            from .bukti import verifikasi_berkas
            return verifikasi_berkas(args.berkas, boleh_takaman=args.boleh_takaman,
                                     direktori_smt=args.smt, verbose=args.verbose)

        if args.mode in ("bangun", "kompilasi"):
            keluaran = args.keluaran
            if keluaran is None:
                dasar = os.path.splitext(os.path.basename(args.berkas))[0]
                keluaran = dasar
            konteks, ukuran_obj = kompilasi(
                args.berkas, keluaran, opt=args.opt, boleh_takaman=args.boleh_takaman,
                strip=args.strip, cc=args.cc, simpan_obj=args.obj, simpan_ir=args.ir)
            for baris, pesan in konteks.peringatan:
                print(f"peringatan baris {baris}: {pesan}", file=sys.stderr)
            ukuran = os.path.getsize(keluaran)
            print(f"ok: {keluaran} ({ukuran} bita, objek {ukuran_obj} bita)")
            return 0

        if args.mode in ("jalan", "jalankan", "interp", "interpretasi"):
            if not args.native:
                return tafsirkan(args.berkas, args.boleh_takaman)
            tmp = tempfile.mkdtemp(prefix="nusa-jalan-")
            exe = os.path.join(tmp, "program")
            kompilasi(args.berkas, exe, opt=args.opt, boleh_takaman=args.boleh_takaman,
                      cc=args.cc)
            arg = list(args.arg)
            if arg and arg[0] == "--":
                arg = arg[1:]
            return subprocess.run([exe] + arg).returncode
    except GalatTafsir as e:
        print(f"galat penafsir: {e}", file=sys.stderr)
        return 1
    except (GalatLex, GalatParse, GalatSemantik) as e:
        _laporkan_galat(e)
        return 1
    except FileNotFoundError as e:
        print(f"galat: berkas tidak ditemukan: {e.filename}", file=sys.stderr)
        return 1
    except RuntimeError as e:
        print(f"galat: {e}", file=sys.stderr)
        return 1

    return 0
