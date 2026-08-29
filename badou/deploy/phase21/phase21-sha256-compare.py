#!/usr/bin/env python3
"""phase21-sha256-compare.py — SHA-256 逐文件比对工具
用法: python phase21-sha256-compare.py <source-dir> <restored-dir> --output <report.json>
退出码: 0=ALL_MATCH, 1=MISMATCH
"""
import hashlib, os, stat, json, sys, argparse

def file_sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

def collect_files(root):
    result = {}
    for dirpath, _, filenames in os.walk(root):
        for fn in filenames:
            full = os.path.join(dirpath, fn)
            rel = os.path.relpath(full, root).replace("\\", "/")
            try:
                st = os.stat(full)
                result[rel] = {
                    "sha256": file_sha256(full),
                    "size": st.st_size,
                    "mode": stat.S_IMODE(st.st_mode),
                    "mtime": int(st.st_mtime),
                }
            except OSError:
                pass
    return result

def main():
    parser = argparse.ArgumentParser(description="SHA-256 逐文件比对")
    parser.add_argument("source", help="源目录")
    parser.add_argument("restored", help="恢复目录")
    parser.add_argument("--output", default="sha256-report.json", help="输出报告")
    args = parser.parse_args()

    src = collect_files(args.source)
    dst = collect_files(args.restored)

    matched, mismatched = 0, []
    all_keys = set(src.keys()) | set(dst.keys())
    for key in sorted(all_keys):
        if key not in src:
            mismatched.append({"path": key, "error": "missing in source"})
        elif key not in dst:
            mismatched.append({"path": key, "error": "missing in restored"})
        elif src[key] != dst[key]:
            mismatched.append({
                "path": key,
                "source": src[key],
                "restored": dst[key],
            })
        else:
            matched += 1

    report = {
        "all_match": len(mismatched) == 0,
        "total_files": len(all_keys),
        "matched": matched,
        "mismatched": mismatched,
    }
    with open(args.output, "w") as f:
        json.dump(report, f, indent=2)

    print(f"Total: {report['total_files']}, Matched: {matched}, Mismatched: {len(mismatched)}")
    sys.exit(0 if report["all_match"] else 1)

if __name__ == "__main__":
    main()