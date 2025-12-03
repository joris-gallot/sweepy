# Basic Named Exports

**Scenario**: A module exports multiple named exports (const, function, class, interface, type, enum).

**Files**:
- `utils.ts`: Contains various named exports

**Test Cases**:
1. **No imports**: All exports should be marked as unused
2. **Some imports**: Only imported exports are used, others remain unused
3. **All imports**: No unused exports

**Expected Behavior**:
- Each named export is tracked individually
- Type-only imports (`import type { MyType }`) should mark the type as used
- Inline type imports (`import { type MyInterface }`) should also work
