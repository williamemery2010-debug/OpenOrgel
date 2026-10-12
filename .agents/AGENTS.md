# Project Rules for OpenOrgel

## Executable Recompilation Requirement
Every time the codebase or synthesis engine is modified, you must compile the standalone release executable and place/overwrite it directly on the user's Desktop.

### Compilation Command:
```powershell
# Compile native Rust Wavetable & FM standalone executable
$llvm = 'C:\Users\EME0012\AppData\Local\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\llvm-mingw-20260602-ucrt-x86_64\bin'
$env:Path = "$env:USERPROFILE\.cargo\bin;$llvm;" + $env:Path
Set-Location rust
cargo build --release
Set-Location ..
```

### Desktop Target Path:
`C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop\openorgelsynth.exe`

### Copy Command:
```powershell
Copy-Item -Path "rust\target\x86_64-pc-windows-gnullvm\release\openorgelsynth.exe" -Destination "C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop\openorgelsynth.exe" -Force
```

## Mandatory Project Timeline & History Logging
For **EVERY commit or change made to the codebase**, you MUST log the change in `project_timeline.txt` with an in-depth technical explanation covering:
- **WHAT**: The specific feature, fix, or architectural change introduced.
- **WHY**: The underlying reason, bug, audio limitation, or musical/computational motivation.
- **HOW**: The technical implementation details, mathematical models/algorithms, data structures, and code flow.

## Code Style
Just get rid of the absurdist comments, it's stupid. Just put simple explanations of whatever engineering is there and what it's doing and why it's being done in that way. For example, if you're doing some random DSP stuff, explain what it's doing in simple terms, don't make up fake chemical compounds or whatever. This is really important for my folio, if there's just weird random shit there that I don't actually understand I'm going to get in trouble.

## GitHub Release & Tagging Workflow
For every change made to the codebase:
1. **Application Compilation & Packaging**:
   - Compile native release binary `openorgelsynth.exe` via `cargo build --release`.
   - Copy `openorgelsynth.exe` to Desktop target path.
2. **Release History & Timeline Documentation**:
   - Update `project_timeline.txt` with in-depth WHAT, WHY, and HOW explanations.
   - Update version history via `python update_history.py`.
   - Stage, commit, and create an annotated git tag (e.g. `Prealpha_v2.0.x`).
   - Push commit and tags to GitHub (`git push origin <branch> --tags`).
