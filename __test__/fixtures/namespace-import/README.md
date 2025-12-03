# Namespace Import

**Scenario**: A module is imported with a namespace import (`import * as utils from './utils'`).

**Files**:
- `utils.ts`: Contains multiple named exports

**Test Cases**:
1. **Namespace import**: `import * as utils from './utils'` should mark all exports as used

**Expected Behavior**:
- Namespace imports (`import * as name`) should mark ALL exports from the module as used
- This is because the consumer can access any export via the namespace object
- Even if specific exports are not referenced in code, they're all potentially accessible
