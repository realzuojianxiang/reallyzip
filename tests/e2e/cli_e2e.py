#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
ReallyZip 命令行端到端自动化测试（无头）。

覆盖：压缩（多文件/单文件/目录/Unicode/跨目录同名/冲突命名/空目录）、解压（extract-here）、
大文件往返、跨工具互操作（Python <-> ReallyZip）、异常与边界、右键菜单注册/注销。

用法：
    python tests/e2e/cli_e2e.py [reallyzip.exe 路径，默认 dist/reallyzip.exe]

退出码：全部通过为 0，存在 FAIL 为 1。结果同时打印到 stdout（供生成报告）。
"""
import os
import sys
import shutil
import hashlib
import subprocess
import tempfile
import zipfile
import winreg

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
EXE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "dist", "reallyzip.exe")
LOCAL = os.environ.get("LOCALAPPDATA", os.path.join(os.path.expanduser("~"), "AppData", "Local"))
INSTALL_DIR = os.path.join(LOCAL, "ReallyZip")

HKCU = winreg.HKEY_CURRENT_USER
results = []


def rec(tid, title, status, detail=""):
    results.append((tid, title, status, detail))
    tag = "PASS" if status else "FAIL"
    line = f"[{tag}] {tid} {title}"
    if detail:
        line += f"  -- {detail}"
    print(line, flush=True)


def run(args):
    return subprocess.run([EXE] + args, capture_output=True, text=True, timeout=120)


def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def snap_zips(folder):
    return set(f for f in os.listdir(folder) if f.lower().endswith(".zip"))


def snap_zips_rec(folder):
    """递归收集目录下所有 .zip 的完整路径（用于多文件压缩时输出落在子目录的情况）。"""
    out = []
    for root, _, files in os.walk(folder):
        for f in files:
            if f.lower().endswith(".zip"):
                out.append(os.path.join(root, f))
    return set(out)


def reg_get(path, name="", root=HKCU):
    try:
        with winreg.OpenKey(root, path) as k:
            return winreg.QueryValueEx(k, name)[0]
    except OSError:
        return None


def reg_exists(path, root=HKCU):
    try:
        winreg.OpenKey(root, path)
        return True
    except OSError:
        return False


def cmd_exe(path):
    """从 command 默认值里取出被引号包裹的 exe 路径。"""
    v = reg_get(path)
    if not v:
        return None
    s = v.find('"')
    if s < 0:
        return None
    r = v[s + 1:].find('"')
    return v[s + 1:s + 1 + r]


# ---------------------------------------------------------------- T-SM-01
def test_smoke():
    ok = os.path.isfile(EXE) and os.path.getsize(EXE) > 1_000_000
    rec("T-SM-01", "发布版产物存在且为单文件(>1MB)", ok,
        f"size={os.path.getsize(EXE)}B" if ok else "缺失或过小")


# ---------------------------------------------------------------- T-CLI-01
def test_multi_file():
    d = tempfile.mkdtemp()
    try:
        p1 = os.path.join(d, "a.txt"); p2 = os.path.join(d, "b.txt")
        os.makedirs(os.path.join(d, "sub"))
        p3 = os.path.join(d, "sub", "c.txt")
        for p, c in [(p1, "A"), (p2, "B"), (p3, "C")]:
            open(p, "w").write(c)
        before = snap_zips(d)
        r = run(["--compress-here", p1, p2, p3])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1
        names = set()
        if ok:
            zp = os.path.join(d, list(new)[0])
            with zipfile.ZipFile(zp) as z:
                names = set(z.namelist())
            # 以公共父目录为根保留相对路径层级（c.txt 在 sub/ 内 → sub/c.txt）
            ok = {"a.txt", "b.txt", "sub/c.txt"} <= names
        rec("T-CLI-01", "多文件 --compress-here 生成单个 zip 含全部", ok,
            f"rc={r.returncode} names={sorted(names)} (保留相对路径)")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-02
def test_single_file():
    d = tempfile.mkdtemp()
    try:
        p = os.path.join(d, "solo.txt")
        open(p, "w").write("hello")
        before = snap_zips(d)
        r = run(["--compress-here", p])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1
        names = set()
        if ok:
            with zipfile.ZipFile(os.path.join(d, list(new)[0])) as z:
                names = set(z.namelist())
            ok = names == {"solo.txt"}
        rec("T-CLI-02", "单文件 --compress-here 生成 <名>.zip", ok,
            f"rc={r.returncode} names={names}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-03
def test_directory():
    d = tempfile.mkdtemp()
    try:
        os.makedirs(os.path.join(d, "tree", "nested"))
        open(os.path.join(d, "tree", "a.txt"), "w").write("A")
        open(os.path.join(d, "tree", "nested", "b.txt"), "w").write("B")
        before = snap_zips(d)
        r = run(["--compress-here", os.path.join(d, "tree")])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1
        names = set()
        if ok:
            with zipfile.ZipFile(os.path.join(d, list(new)[0])) as z:
                names = set(z.namelist())
            ok = {"tree/", "tree/a.txt", "tree/nested/", "tree/nested/b.txt"} <= names
        rec("T-CLI-03", "目录压缩保留顶层文件夹名(tree/...)", ok,
            f"rc={r.returncode} names={sorted(names)}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-04
def test_unicode_names():
    d = tempfile.mkdtemp()
    try:
        f1 = os.path.join(d, "文件 A.txt")
        sub = os.path.join(d, "目录 B")
        os.makedirs(sub)
        f2 = os.path.join(sub, "内容 中文.txt")
        open(f1, "w", encoding="utf-8").write("中文内容")
        open(f2, "w", encoding="utf-8").write("更多中文")
        before = snap_zips(d)
        r = run(["--compress-here", f1, f2])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1
        names = set()
        if ok:
            with zipfile.ZipFile(os.path.join(d, list(new)[0])) as z:
                names = set(z.namelist())
            ok = ("文件 A.txt" in names) and ("目录 B/内容 中文.txt" in names)
        rec("T-CLI-04", "空格/中文/Unicode 文件名无损(保留相对路径)", ok,
            f"rc={r.returncode} names={sorted(names)}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-05
def test_collision():
    d = tempfile.mkdtemp()
    try:
        p = os.path.join(d, "dup.txt")
        open(p, "w").write("x")
        # 预先占用目标名 dup.zip
        open(os.path.join(d, "dup.zip"), "w").write("")
        before = snap_zips(d)
        r = run(["--compress-here", p])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1 and list(new)[0] == "dup (2).zip"
        rec("T-CLI-05", "目标已存在时自动命名 (2).zip", ok,
            f"rc={r.returncode} new={sorted(new)}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-06
def test_large_roundtrip():
    d = tempfile.mkdtemp()
    try:
        big = os.path.join(d, "big.bin")
        data = os.urandom(50 * 1024 * 1024)
        open(big, "wb").write(data)
        before = snap_zips(d)
        r1 = run(["--compress-here", big])
        new = snap_zips(d) - before
        ok = r1.returncode == 0 and len(new) == 1
        if ok:
            zp = os.path.join(d, list(new)[0])
            r2 = run(["--extract-here", zp])
            ext = os.path.join(d, "big", "big.bin")
            ok = r2.returncode == 0 and os.path.isfile(ext) and sha(ext) == sha(big)
        rec("T-CLI-06", "大文件(50MB)压缩->解压内容一致", ok,
            f"rc1={r1.returncode} rc2={r2.returncode if ok else '?'}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-07 / 08
def test_extract_here():
    d = tempfile.mkdtemp()
    try:
        zp = os.path.join(d, "pack.zip")
        with zipfile.ZipFile(zp, "w") as z:
            z.writestr("top.txt", "T")
            z.writestr("lvl1/deep.txt", "D")
        r = run(["--extract-here", zp])
        folder = os.path.join(d, "pack")
        ok = r.returncode == 0 and os.path.isfile(os.path.join(folder, "top.txt")) \
            and os.path.isfile(os.path.join(folder, "lvl1", "deep.txt"))
        rec("T-CLI-07/08", "extract-here 到同名文件夹且保留层级", ok,
            f"rc={r.returncode}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-09
def test_interop_rz_to_py():
    d = tempfile.mkdtemp()
    try:
        p1 = os.path.join(d, "x.txt"); p2 = os.path.join(d, "y.txt")
        open(p1, "w").write("X" * 1000); open(p2, "w").write("Y" * 500)
        before = snap_zips(d)
        r = run(["--compress-here", p1, p2])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1
        if ok:
            with zipfile.ZipFile(os.path.join(d, list(new)[0])) as z:
                bad = z.testzip()  # CRC 校验
                ok = bad is None
        rec("T-CLI-09", "互操作: ReallyZip 包可被 Python 正确读取(CRC)", ok,
            f"rc={r.returncode} testzip={bad if ok else '?'}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-10
def test_interop_py_to_rz():
    d = tempfile.mkdtemp()
    try:
        zp = os.path.join(d, "frompy.zip")
        src = {"a.txt": b"AAA", "dir/b.txt": b"BBB"}
        with zipfile.ZipFile(zp, "w", zipfile.ZIP_DEFLATED) as z:
            for n, c in src.items():
                z.writestr(n, c)
        r = run(["--extract-here", zp])
        folder = os.path.join(d, "frompy")
        ok = r.returncode == 0
        if ok:
            got = {}
            for n in src:
                fp = os.path.join(folder, n)
                if os.path.isfile(fp):
                    got[n] = open(fp, "rb").read()
            ok = got == src
        rec("T-CLI-10", "互操作: Python 生成的 zip 被 ReallyZip 解压还原", ok,
            f"rc={r.returncode} got={sorted(got) if ok else '?'}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-11
def test_error_nonexistent():
    d = tempfile.mkdtemp()
    try:
        before = snap_zips(d)
        r = run(["--compress-here", os.path.join(d, "ghost.txt")])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 0  # 无产物、无崩溃
        rec("T-CLI-11", "异常: 不存在的路径优雅退出无产物", ok,
            f"rc={r.returncode} new={sorted(new)}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-12
def test_error_not_zip():
    d = tempfile.mkdtemp()
    try:
        notzip = os.path.join(d, "bad.zip")
        open(notzip, "w").write("this is not a zip")
        r = run(["--extract-here", notzip])
        folder = os.path.join(d, "bad")
        ok = r.returncode == 0 and not os.path.isdir(folder)  # 不崩溃、不产生目录
        rec("T-CLI-12", "异常: 对非 zip 调 extract-here 不崩溃", ok,
            f"rc={r.returncode} created_folder={os.path.isdir(folder)}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-13
def test_empty_dir():
    d = tempfile.mkdtemp()
    try:
        os.makedirs(os.path.join(d, "emptydir"))
        before = snap_zips(d)
        r = run(["--compress-here", os.path.join(d, "emptydir")])
        new = snap_zips(d) - before
        ok = r.returncode == 0 and len(new) == 1
        if ok:
            with zipfile.ZipFile(os.path.join(d, list(new)[0])) as z:
                names = set(z.namelist())
            ok = any(n == "emptydir/" or n.startswith("emptydir/") for n in names)
        rec("T-CLI-13", "边界: 空目录压缩不崩溃且含目录项", ok,
            f"rc={r.returncode}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-CLI-14 (F-01 回归)
def test_crossdir_collision():
    """不同目录里同名文件不应互相覆盖（F-01 修复验证）。"""
    d = tempfile.mkdtemp()
    try:
        dir_a = os.path.join(d, "folder_a")
        dir_b = os.path.join(d, "folder_b")
        os.makedirs(dir_a); os.makedirs(dir_b)
        pa = os.path.join(dir_a, "report.txt")
        pb = os.path.join(dir_b, "report.txt")
        open(pa, "w", encoding="utf-8").write("内容-A")
        open(pb, "w", encoding="utf-8").write("内容-B")
        before = snap_zips_rec(d)
        r = run(["--compress-here", pa, pb])
        new = snap_zips_rec(d) - before
        ok = r.returncode == 0 and len(new) == 1
        names = set()
        if ok:
            with zipfile.ZipFile(list(new)[0]) as z:
                names = set(z.namelist())
            # 公共祖先为 d，因此保留相对路径 folder_a/report.txt 与 folder_b/report.txt
            ok = {"folder_a/report.txt", "folder_b/report.txt"} <= names
            # 且两者内容都能独立还原
            if ok:
                with zipfile.ZipFile(os.path.join(d, list(new)[0])) as z:
                    ca = z.read("folder_a/report.txt").decode("utf-8")
                    cb = z.read("folder_b/report.txt").decode("utf-8")
                ok = (ca == "内容-A") and (cb == "内容-B")
        rec("T-CLI-14", "F-01: 跨目录同名文件保留相对路径不覆盖", ok,
            f"rc={r.returncode} names={sorted(names)}")
    finally:
        shutil.rmtree(d, ignore_errors=True)


# ---------------------------------------------------------------- T-SH-01/02/03
def test_shell():
    # 先确保干净
    run(["--unregister-shell"])
    # T-SH-01 注册
    r = run(["--register-shell"])
    ok1 = r.returncode == 0
    ok1 &= reg_exists(r"Software\Classes\*\shell\ReallyZip")
    ok1 &= reg_get(r"Software\Classes\*\shell\ReallyZip", "MUIVerb") == "ReallyZip"
    ok1 &= reg_get(r"Software\Classes\*\shell\ReallyZip", "ExtendedSubCommandsKey") == "ReallyZip.FileMenu"
    ok1 &= reg_exists(r"Software\Classes\ReallyZip.FileMenu\shell\01Add\command")
    ok1 &= reg_exists(r"Software\Classes\ReallyZip.FileMenu\shell\02AddTo\command")
    exe_a = cmd_exe(r"Software\Classes\ReallyZip.FileMenu\shell\02AddTo\command")
    expected = os.path.join(INSTALL_DIR, "reallyzip.exe").replace("/", "\\")
    ok1 &= (exe_a is not None) and (exe_a.lower() == expected.lower())
    ok1 &= os.path.isfile(expected)
    ok1 &= reg_exists(r"Software\Classes\ReallyZip.ZipMenu\shell\01Open")
    ok1 &= reg_exists(r"Software\Classes\ReallyZip.ZipMenu\shell\04Add")
    rec("T-SH-01", "注册: 级联入口+子菜单齐全, 命令指向稳定路径", ok1,
        f"rc={r.returncode} exe={exe_a}")
    # T-SH-03 重复注册幂等
    r2 = run(["--register-shell"])
    n_sub1 = len([k for k in
                  [reg_exists(r"Software\Classes\ReallyZip.FileMenu\shell\01Add"),
                   reg_exists(r"Software\Classes\ReallyZip.FileMenu\shell\02AddTo")] if k])
    ok3 = r2.returncode == 0 and n_sub1 == 2 and os.path.isfile(expected)
    rec("T-SH-03", "重复注册无孤儿/无重复键", ok3, f"rc={r2.returncode}")
    # T-SH-02 注销清理
    r3 = run(["--unregister-shell"])
    ok2 = r3.returncode == 0
    ok2 &= not reg_exists(r"Software\Classes\*\shell\ReallyZip")
    ok2 &= not reg_exists(r"Software\Classes\ReallyZip.FileMenu")
    ok2 &= not reg_exists(r"Software\Classes\ReallyZip.ZipMenu")
    ok2 &= not os.path.isdir(INSTALL_DIR)
    rec("T-SH-02", "注销: 注册表键与安装目录全部清除", ok2, f"rc={r3.returncode}")


def main():
    if not os.path.isfile(EXE):
        print(f"ERROR: 找不到被测对象 {EXE}")
        sys.exit(2)
    print(f"被测对象: {EXE}  ({os.path.getsize(EXE)} B)")
    print("=" * 72)
    test_smoke()
    test_multi_file()
    test_single_file()
    test_directory()
    test_unicode_names()
    test_collision()
    test_large_roundtrip()
    test_extract_here()
    test_interop_rz_to_py()
    test_interop_py_to_rz()
    test_error_nonexistent()
    test_error_not_zip()
    test_empty_dir()
    test_crossdir_collision()
    test_shell()
    print("=" * 72)
    passed = sum(1 for _, _, s, _ in results if s)
    failed = len(results) - passed
    print(f"SUMMARY: {passed} passed, {failed} failed, total {len(results)}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    try:
        main()
    finally:
        # 无论如何收尾，确保不残留右键菜单注册
        try:
            subprocess.run([EXE, "--unregister-shell"], capture_output=True, timeout=60)
        except Exception:
            pass
