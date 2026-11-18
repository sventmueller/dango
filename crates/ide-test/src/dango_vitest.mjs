// Dango's Vitest reporter: each test's start and result as a JSON line in the file DANGO_JS_EVENTS names, with its
// file, its name as `vitest list` writes it, its line and a failure's message and stack.

import { appendFileSync } from "node:fs";

function emit(record) {
  appendFileSync(process.env.DANGO_JS_EVENTS, JSON.stringify(record) + "\n");
}

function failure(errors) {
  const error = (errors || [])[0];
  return error ? { text: error.stack || error.message || String(error) } : {};
}

const OUTCOMES = { passed: "passed", failed: "failed", skipped: "skipped", pass: "passed", fail: "failed", skip: "skipped", todo: "skipped" };

export default class DangoReporter {
  constructor() {
    this.caseHooks = false;
    this.pattern = null;
  }

  onInit(vitest) {
    this.pattern = vitest.config?.testNamePattern ?? null;
  }

  // A test the name filter left out keeps what an earlier run said of it.
  chosen(name) {
    return !this.pattern || this.pattern.test(name);
  }

  who(testCase) {
    return { file: testCase.module.moduleId, name: testCase.fullName, line: testCase.location?.line };
  }

  onTestCaseReady(testCase) {
    this.caseHooks = true;
    if (!this.chosen(testCase.fullName)) {
      return;
    }
    emit({ event: "started", ...this.who(testCase) });
  }

  onTestCaseResult(testCase) {
    this.caseHooks = true;
    const result = testCase.result();
    const outcome = OUTCOMES[result.state];
    if (outcome && this.chosen(testCase.fullName)) {
      emit({ event: "result", ...this.who(testCase), outcome, ...(outcome === "failed" ? failure(result.errors) : {}) });
    }
  }

  // Vitest before 3 has no hooks per test case; its results come once the run ended.
  onFinished(files) {
    if (this.caseHooks) {
      return;
    }
    const walk = (task, path, file) => {
      if (task.tasks) {
        for (const child of task.tasks) {
          walk(child, task.type === "suite" ? [...path, task.name] : path, file);
        }
        return;
      }
      const outcome = OUTCOMES[task.result?.state ?? task.mode];
      const name = [...path, task.name].join(" > ");
      if (outcome && this.chosen(name)) {
        emit({
          event: "result",
          file,
          name,
          line: task.location?.line,
          outcome,
          ...(outcome === "failed" ? failure(task.result?.errors) : {}),
        });
      }
    };
    for (const file of files || []) {
      walk(file, [], file.filepath);
    }
  }
}
