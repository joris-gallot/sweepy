# Basic Default Exports

**Scenario**: A module exports a default export along with named exports.

**Files**:
- `utils.ts`: Contains a default export (function) and a named export

**Test Cases**:
1. **No imports**: Both default and named exports should be marked as unused
2. **Default import only**: Default is used, named export is unused
3. **Named import only**: Named export is used, default is unused
4. **Mixed import**: Both default and named are used (`import foo, { namedExport } from './utils'`)

**Expected Behavior**:
- Default exports are tracked under the special name `"default"`
- Default imports can use any local name (`import anything from './utils'`)
- Mixed imports should mark both as used
