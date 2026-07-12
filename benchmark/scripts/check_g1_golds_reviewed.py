#!/usr/bin/env python3
import json, sys
from pathlib import Path
m = json.loads(Path("benchmark/real_track/manifests/real_structure_v0.json").read_text())
errs = []
for d in m["documents"]:
    p = Path("benchmark/real_track") / (d.get("gold_path") or f"gold/{d['id']}.json")
    g = json.loads(p.read_text())
    if g.get("status") != "reviewed":
        errs.append((d["id"], g.get("status")))
if errs:
    print("not reviewed:", errs)
    sys.exit(1)
print(f"OK {len(m['documents'])} reviewed")
