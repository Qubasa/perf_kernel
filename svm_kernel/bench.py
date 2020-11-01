#!/usr/bin/env python3
from subprocess import Popen, PIPE
import sys

cmd = f"cargo test --test {sys.argv[1]}"
print(cmd)

res = {}
for _ in range(5):
    p = Popen(cmd, shell=True, stdout=PIPE)
    output = p.stdout.read().decode()
    output = filter(lambda s: s.startswith("Cycles needed:"), output.split("\n"))
    for (i, cycle) in enumerate(output):
        if res.get(i) is None:
            res[i] = []
        res[i].append(cycle.split(" ")[2])
        res[i].sort()

for i in res.values():
    print(i)
    print(i[int(len(i)/2)])
