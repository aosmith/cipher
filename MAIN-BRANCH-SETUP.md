# Main Branch Setup

The repository has been set up with `main` as the primary development branch.

## Changes Made

- Created `main` branch from `cipher` branch
- Pushed `main` branch to origin
- All latest code including macOS desktop app release is now on `main`

## Manual GitHub Step Required

**Action Needed**: Set `main` as the default branch on GitHub:

1. Go to https://github.com/aosmith/cipher/settings/branches
2. Change default branch from `cipher` to `main`
3. Delete the old `cipher` branch after confirming the switch

## Branch Status

- ✅ `main` branch: Contains all latest code and releases
- ⚠️ `cipher` branch: Can be deleted after default branch change
- ✅ Repository URLs: No changes needed (still `aosmith/cipher`)

## For Contributors

Going forward, use:
```bash
git clone https://github.com/aosmith/cipher.git
cd cipher
# Will automatically check out main branch
```

The repository structure and URLs remain the same - only the default branch name has changed from `cipher` to `main` for standard Git conventions.