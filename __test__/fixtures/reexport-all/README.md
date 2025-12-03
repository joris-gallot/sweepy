# Re-export All

**Scenario**: A barrel file re-exports all exports from another module using `export *`.

**Files**:
- `barrel.ts`: Re-exports everything from `utils.ts` plus an additional export
- `utils.ts`: Contains multiple named exports

**Test Cases**:
1. **No imports**: All exports from both files should be marked as unused
2. **Import from barrel**: Importing specific exports from barrel should mark those as used in utils
3. **Namespace import from barrel**: Should mark all exports as used (from both barrel and utils)

**Expected Behavior**:
- `export *` creates a re-export relationship where exports are accessible through the barrel
- Importing from the barrel should trace back to the original module
- The `extra` export in barrel is independent and tracked separately
