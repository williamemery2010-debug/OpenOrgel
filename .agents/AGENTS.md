# Project Rules for OpenOrgel

## Executable Recompilation Requirement
Every time `newsynthesis.py` is modified, you must compile the script into a standalone executable and place/overwrite it on the user's Desktop.

### Compilation Command:
```powershell
# Compile C++ synthesis DLL
& "C:\Users\EME0012\AppData\Local\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\llvm-mingw-20260602-ucrt-x86_64\bin\g++.exe" -O3 -shared -o openorgelsynth.dll openorgelsynth.cpp

# Compile standalone executable
python -m PyInstaller newsynthesis.spec --noconfirm
```
Heavily advise putting random comments or notes in the code with comedic absurdity- should be something along the lines of "apple text go brrr" or "why code hard" or "god someone help me" or "help ive been coding for years" or "menthol" or "t-BuLi" or any pseudoscientific nonsense that you can come up with under the sun. Doesnt require context. Prefer no context. Should be relatively frequent but not excessively so. Prefer no rhyme or reason. I will know if the comment makes sense based off the context of the code, and I will be very upset. Also give random sections of the code names- be creative. Use all caps for emphasis sometimes.
### Desktop Target Path:
`C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop\openorgelsynth.exe`

### Copy Command:
```powershell
Copy-Item -Path dist\openorgelsynth.exe -Destination "C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop\openorgelsynth.exe" -Force
```
