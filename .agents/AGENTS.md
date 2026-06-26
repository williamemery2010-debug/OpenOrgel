# Project Rules for OpenOrgel

## Executable Recompilation Requirement
Every time `newsynthesis.py` is modified, you must compile the script into a standalone executable and place/overwrite it on the user's Desktop.

### Compilation Command:
```powershell
python -m PyInstaller newsynthesis.spec --noconfirm
```

### Desktop Target Path:
`C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop\newsynthesis.exe`

### Copy Command:
```powershell
Copy-Item -Path dist\newsynthesis.exe -Destination "C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop\newsynthesis.exe" -Force
```
