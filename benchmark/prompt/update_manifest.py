import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("suite", choices=["smoke", "practice"])
    args = parser.parse_args()

    case_path = ROOT / args.suite / "cases.json"
    payload = case_path.read_bytes()
    cases = json.loads(payload.decode("utf-8"))
    categories = {}
    depths = {}
    for case in cases:
        if case.get("suite") != args.suite:
            raise RuntimeError(
                f"{case.get('id', '<unknown>')} has suite {case.get('suite')!r}"
            )
        category = case["category"]
        depth = str(case["depth"])
        categories[category] = categories.get(category, 0) + 1
        depths[depth] = depths.get(depth, 0) + 1
    manifest = {
        "schema_version": 1,
        "suite": args.suite,
        "case_file": "cases.json",
        "sha256": hashlib.sha256(payload).hexdigest(),
        "total": len(cases),
        "category_counts": categories,
        "depth_counts": depths,
    }
    output = ROOT / args.suite / "manifest.json"
    output.write_bytes(
        (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode(
            "utf-8"
        )
    )
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
