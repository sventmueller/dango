"""Dango's pytest plugin: each collected test, each start and each result as a JSON line in the file that
DANGO_PYTEST_EVENTS names, with the test's node id, file and line."""

import json
import os

_OUT = None


def _emit(record):
    global _OUT
    if _OUT is None:
        _OUT = open(os.environ["DANGO_PYTEST_EVENTS"], "a", encoding="utf-8")
    _OUT.write(json.dumps(record) + "\n")
    _OUT.flush()


def pytest_collection_modifyitems(session, config, items):
    for item in items:
        path, line, _ = item.location
        _emit({"event": "collected", "id": item.nodeid, "path": path, "line": (line or 0) + 1})


def pytest_runtest_logstart(nodeid, location):
    _emit({"event": "started", "id": nodeid})


def pytest_runtest_logreport(report):
    if report.when == "call" or (report.when == "setup" and report.outcome != "passed"):
        record = {"event": "result", "id": report.nodeid, "outcome": report.outcome}
        crash = getattr(getattr(report, "longrepr", None), "reprcrash", None)
        if report.failed and crash is not None:
            record.update({"path": crash.path, "line": crash.lineno, "message": crash.message})
        elif report.skipped and isinstance(report.longrepr, tuple):
            record["message"] = report.longrepr[2]
        _emit(record)
