// Dango's Jest reporter: each test's start and result as a JSON line in the file DANGO_JS_EVENTS names, with its
// file, its describe blocks and title joined with ` > `, its line and a failure's message with the stack.

const { appendFileSync } = require("node:fs");

function emit(record) {
  appendFileSync(process.env.DANGO_JS_EVENTS, JSON.stringify(record) + "\n");
}

const OUTCOMES = { passed: "passed", failed: "failed", pending: "skipped", skipped: "skipped", todo: "skipped", disabled: "skipped" };

function name(info) {
  return [...(info.ancestorTitles || []), info.title].join(" > ");
}

class DangoReporter {
  constructor(globalConfig) {
    this.written = new Set();
    const pattern = globalConfig?.testNamePattern;
    this.pattern = pattern ? new RegExp(pattern, "i") : null;
  }

  // A test the name filter left out keeps what an earlier run said of it; Jest matches the filter against the
  // describe blocks and title joined with spaces.
  chosen(info) {
    return !this.pattern || this.pattern.test([...(info.ancestorTitles || []), info.title].join(" "));
  }

  onTestCaseStart(test, info) {
    if (!this.chosen(info)) {
      return;
    }
    emit({ event: "started", file: test.path, name: name(info) });
  }

  onTestCaseResult(test, result) {
    this.result(test.path, result);
  }

  // A Jest without hooks per test case still reports each file's results here once the file ran.
  onTestResult(test, testResult) {
    for (const result of testResult.testResults || []) {
      this.result(test.path, result);
    }
  }

  result(file, result) {
    const key = file + "\0" + name(result);
    const outcome = OUTCOMES[result.status];
    if (!outcome || this.written.has(key) || !this.chosen(result)) {
      return;
    }
    this.written.add(key);
    const record = { event: "result", file, name: name(result), line: result.location?.line, outcome };
    if (outcome === "failed" && result.failureMessages?.length) {
      record.text = result.failureMessages[0];
    }
    emit(record);
  }
}

module.exports = DangoReporter;
