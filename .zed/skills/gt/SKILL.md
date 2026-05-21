---
name: gt
description: Git add, commit ve push işlemini yapar. Semgrep hook hatası varsa düzeltip retry eder.
---

# Git Commit Push

## Workflow

1. Run `git add .`.

2. If there are no changes to commit, output exactly `Hiç değişiklik yok` and stop.

3. Do not run Semgrep manually.

4. Run `git commit -m "<Türkçe, belirli ve emir kipinde mesaj>"`.

5. If commit or push hook reports a Semgrep error or warning:
   - Find the affected file and line.
   - Apply the smallest safe fix.
   - Do not refactor unrelated code.
   - Run `git add .`.
   - Retry the failed commit or push step.

6. If the Semgrep issue cannot be fixed safely, stop and report it clearly.

7. After commit succeeds, run `git push`.

8. Do not run or print unrelated commands.
