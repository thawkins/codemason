---
status: draft
spec_id: exasearch
---

# Manual Test Plan — Exa Search API Backend

## Prerequisites

1. **Exa API key** — obtain one from
   [https://dashboard.exa.ai/api-keys](https://dashboard.exa.ai/api-keys).
2. **codemason.json** — add the key to the project or global config:

   ```jsonc
   {
     "exa_api_key": "your-exa-api-key-here"
   }
   ```

   Alternatively, export the environment variable:

   ```bash
   export EXA_API_KEY="your-exa-api-key-here"
   ```

3. **Build codemason** — run `cargo build` from the project root.
4. **Terminal** — open a terminal in the project root directory.

## Test Cases

### TC-001: Exa backend appears in engine status when API key is configured

**Preconditions:**
- `exa_api_key` is set in `codemason.json` or `EXA_API_KEY` is in the environment.
- codemason is built and the TUI is not yet running.

**Steps:**
1. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
2. Type the following slash command and press Enter:
   ```
   /websearch show
   ```
3. Observe the engine status list printed in the chat panel.

**Expected results:**
- The output includes a line for `Exa` with:
  - `enabled: true`
  - `in_use: true`
  - `failed: false`

---

### TC-002: Exa backend marked as failed when API key is missing

**Preconditions:**
- No `exa_api_key` in `codemason.json` and `EXA_API_KEY` is **not** set in the
  environment.
- codemason is built.

**Steps:**
1. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
2. Type the following slash command and press Enter:
   ```
   /websearch show
   ```
3. Observe the engine status list.

**Expected results:**
- The output includes a line for `Exa` with:
  - `enabled: false`
  - `in_use: false`
  - `failed: true`

---

### TC-003: Search using the `exa` engine restriction returns results

**Preconditions:**
- `exa_api_key` is set in `codemason.json` or `EXA_API_KEY` is in the environment.
- codemason is built and the TUI is running.

**Steps:**
1. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
2. Type the following message and press Enter:
   ```
   Use the mf_search tool to search for "rust async programming" with engine set to "exa" and max_results 5.
   ```
3. Wait for the agent to execute the tool and display results.

**Expected results:**
- The tool output shows results from the `exa` engine only.
- Each result has:
  - A title
  - A URL
  - A snippet (from Exa highlights or metadata)
  - `source: exa`
  - A `relevance_score` value
- The `Blocked engines` line does **not** list `exa`.

---

### TC-004: Default parallel search includes Exa when configured

**Preconditions:**
- `exa_api_key` is set in `codemason.json` or `EXA_API_KEY` is in the environment.
- codemason is built and the TUI is running.

**Steps:**
1. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
2. Type the following message and press Enter:
   ```
   Use the mf_search tool to search for "machine learning frameworks 2024" with max_results 10. Do not set the engine parameter.
   ```
3. Wait for the agent to execute the tool and display results.

**Expected results:**
- The tool output shows results merged from multiple engines.
- The `Engines:` metadata line reports a count that includes the Exa backend.
- At least one result has `source: exa` (visible in the structured metadata or
  the raw result breakdown).

---

### TC-005: API key is never logged in plain text

**Preconditions:**
- `exa_api_key` is set to a known value (e.g. `abcdef12-3456-7890-abcd-ef1234567890`).
- codemason is built with log level set to `debug`:
  ```
  ./target/debug/cdmn --log-level debug
  ```

**Steps:**
1. Launch codemason with debug logging:
   ```
   ./target/debug/cdmn --log-level debug
   ```
2. Type the following message and press Enter:
   ```
   Use the mf_search tool to search for "test query" with engine set to "exa".
   ```
3. Check the terminal output and any log files for the API key string.

**Expected results:**
- The full API key `abcdef12-3456-7890-abcd-ef1234567890` does **not** appear
  anywhere in the log output.
- If the key is referenced at all, it appears in masked form
  (e.g. `ab***************************7890`).

---

### TC-006: Exa backend gracefully degrades on invalid API key

**Preconditions:**
- `EXA_API_KEY` is set to an obviously invalid value (e.g. `invalid-key-123`).
- codemason is built.

**Steps:**
1. Set the invalid key:
   ```bash
   export EXA_API_KEY="invalid-key-123"
   ```
2. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
3. Type the following message and press Enter:
   ```
   Use the mf_search tool to search for "test" with engine set to "exa".
   ```
4. Observe the tool output.

**Expected results:**
- The tool does **not** crash or hang.
- The `Blocked engines` line lists `exa` with an auth-failure or blocked
  message.
- No results are attributed to `exa`.

---

### TC-007: Engine enum in tool schema accepts "exa"

**Preconditions:**
- codemason is built.
- `exa_api_key` is set.

**Steps:**
1. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
2. Type the following message and press Enter:
   ```
   Call the mf_search tool with query "hello world", engine "exa", max_results 3.
   ```
3. Observe whether the tool call is accepted and executed.

**Expected results:**
- The tool call is accepted (no schema-validation error about an invalid
  `engine` enum value).
- Results are returned from the Exa backend.

---

### TC-008: Freshness filter maps to Exa date parameters

**Preconditions:**
- `exa_api_key` is set.
- codemason is built.

**Steps:**
1. Launch codemason in TUI mode:
   ```
   ./target/debug/cdmn
   ```
2. Type the following message and press Enter:
   ```
   Use the mf_search tool to search for "AI news" with engine "exa", freshness "week", max_results 5.
   ```
3. Observe the results.

**Expected results:**
- Results are returned from the Exa backend.
- Returned results have `publishedDate` values within the last week (or are
  recent enough that the date filter was applied). The tool does not error.

## Cleanup

1. Remove the test `exa_api_key` from `codemason.json` if one was added for
   testing.
2. Unset the environment variable:
   ```bash
   unset EXA_API_KEY
   ```
3. Exit codemason (press `Ctrl+C` or type `/quit`).