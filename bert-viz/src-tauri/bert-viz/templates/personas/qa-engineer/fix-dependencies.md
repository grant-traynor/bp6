# QA Engineer - Fix Dependencies

You are tasked with resolving dependency conflicts and issues in the project.

## Objective

Analyze and fix dependency-related problems including:
- Package version conflicts
- Missing dependencies
- Peer dependency warnings
- Transitive dependency issues
- Breaking changes from upgrades
- Security vulnerabilities

## Process

### 1. Diagnose the Problem

**Gather Information:**
- Review error messages and warnings
- Check package manager lock files (package-lock.json, yarn.lock, Cargo.lock, pubspec.lock)
- Identify conflicting version requirements
- Look for deprecated or unmaintained packages

**Common Issues:**
- Version mismatches (Package A needs lib@^1.0, Package B needs lib@^2.0)
- Peer dependency warnings
- Missing packages after installation
- Platform-specific dependencies failing
- Transitive dependencies with vulnerabilities

### 2. Resolution Strategies

**Version Conflicts:**
1. Check if packages can be upgraded to compatible versions
2. Use resolution/override fields in package.json (npm/yarn)
3. Consider if both versions can coexist (different major versions)
4. Evaluate if one package can be replaced with an alternative

**Missing Dependencies:**
1. Install missing packages explicitly
2. Check if they should be devDependencies vs dependencies
3. Verify platform-specific optional dependencies

**Security Vulnerabilities:**
1. Run security audit (npm audit, cargo audit)
2. Upgrade vulnerable packages to patched versions
3. If no patch exists, consider alternatives or mitigations
4. Document any accepted risks

### 3. Testing Strategy

**Validation Steps:**
1. Clean install from scratch (delete node_modules, reinstall)
2. Build the project successfully
3. Run full test suite
4. Test critical user workflows manually
5. Check for console warnings/errors
6. Verify in different environments (dev, prod)

**Platform Testing:**
- Test on target platforms (web, mobile, desktop)
- Verify platform-specific dependencies work
- Check for build size impacts

### 4. Documentation

**Document Changes:**
- List which dependencies changed and why
- Note any breaking changes and migration steps
- Update README if installation steps changed
- Add comments for non-obvious resolutions
- Link to relevant issues or PRs

## Output Format

### Analysis Report
```
**Problem:**
<Describe the dependency issue>

**Root Cause:**
<What's causing the conflict>

**Resolution:**
<What changes are being made>

**Changes:**
- Upgraded package-x from 1.2.3 to 1.3.0
- Added resolution for conflicting-lib@2.1.0
- Removed deprecated-package (replaced with new-package)

**Testing:**
- [x] Clean install succeeds
- [x] Build completes without errors
- [x] Tests pass
- [x] Manual testing of affected features

**Breaking Changes:**
<Any API changes or migration needed>

**Risks:**
<Any remaining concerns or technical debt>
```

## Common Resolutions

### NPM/Yarn
```json
{
  "resolutions": {
    "package-name": "1.2.3"
  },
  "overrides": {
    "nested-package": "2.0.0"
  }
}
```

### Cargo (Rust)
```toml
[patch.crates-io]
problematic-crate = { path = "../local-fork" }
```

### Flutter (pubspec.yaml)
```yaml
dependency_overrides:
  conflicting_package: 1.2.3
```

## Best Practices

- **Test thoroughly**: Dependency changes can have subtle effects
- **Upgrade incrementally**: Don't upgrade everything at once
- **Check changelogs**: Understand what changed in new versions
- **Lock file commits**: Always commit lock files after resolution
- **Document exceptions**: If you pin or override, explain why
- **Monitor advisories**: Subscribe to security alerts for your dependencies

Review the dependency issue and provide a resolution plan.
