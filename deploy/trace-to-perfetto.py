#!/usr/bin/env python3
"""Convert a Tempo trace to a file you can open in https://ui.perfetto.dev.

    ./trace-to-perfetto.py <trace_id | trace.json> [out.json]

Takes a trace id (downloaded from Tempo) or a JSON file exported from Grafana.
Writes Chrome trace-event JSON: one process per service, and one row per set of
spans that do not overlap, so parallel spans show side by side.
"""

import json
import os
import sys
import urllib.request

TEMPO = os.environ.get("TEMPO_URL", "http://localhost:3200")


def load(arg):
    if os.path.isfile(arg):
        with open(arg) as f:
            return json.load(f)
    request = urllib.request.Request(f"{TEMPO}/api/traces/{arg}", headers={"Accept": "application/json"})
    return json.load(urllib.request.urlopen(request))


def attr_value(value):
    return next(iter(value.values()), "") if value else ""


def spans_by_service(trace):
    services = {}
    for batch in trace.get("batches") or trace.get("resourceSpans") or []:
        attrs = {a["key"]: attr_value(a["value"]) for a in batch["resource"].get("attributes", [])}
        service = attrs.get("service.name", "unknown")
        for scope in batch.get("scopeSpans", []):
            for span in scope.get("spans", []):
                services.setdefault(service, []).append(span)
    return services


def to_lanes(spans):
    """Put each span on the first row where it does not overlap a sibling.

    A row is a stack of open spans: a span fits if the row is free at its start,
    or if it fits inside the span on top of the stack (a child).
    """
    lanes = []
    placed = []
    for span in sorted(spans, key=lambda s: (int(s["startTimeUnixNano"]), -int(s["endTimeUnixNano"]))):
        start, end = int(span["startTimeUnixNano"]), int(span["endTimeUnixNano"])
        for i, stack in enumerate(lanes):
            while stack and stack[-1] <= start:
                stack.pop()
            if not stack or end <= stack[-1]:
                break
        else:
            lanes.append([])
            i = len(lanes) - 1
        lanes[i].append(end)
        placed.append((i, span))
    return placed


def convert(trace):
    services = spans_by_service(trace)
    t0 = min(int(s["startTimeUnixNano"]) for spans in services.values() for s in spans)
    events = []
    for pid, (service, spans) in enumerate(sorted(services.items()), start=1):
        events.append({"ph": "M", "name": "process_name", "pid": pid, "args": {"name": service}})
        for tid, span in to_lanes(spans):
            start, end = int(span["startTimeUnixNano"]), int(span["endTimeUnixNano"])
            args = {a["key"]: attr_value(a["value"]) for a in span.get("attributes", [])}
            events.append({
                "ph": "X",
                "name": span["name"],
                "pid": pid,
                "tid": tid,
                "ts": (start - t0) / 1000,  # microseconds
                "dur": (end - start) / 1000,
                "args": args,
            })
    return {"traceEvents": events, "displayTimeUnit": "ms"}


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    arg = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else f"{os.path.basename(arg).removesuffix('.json')}.perfetto.json"
    with open(out, "w") as f:
        json.dump(convert(load(arg)), f)
    print(f"wrote {out}, open it in https://ui.perfetto.dev")


if __name__ == "__main__":
    main()
