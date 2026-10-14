import os
import shutil
import zipfile

def build_package():
    dist_dir = "Open-Orgel Software"
    desktop_dir = r"C:\Users\EME0012\OneDrive - St Helena Secondary College\Desktop"
    
    if os.path.exists(dist_dir):
        shutil.rmtree(dist_dir)
        
    os.makedirs(dist_dir, exist_ok=True)
    
    # Subfolder 1: Engine_and_Source
    engine_dir = os.path.join(dist_dir, "Engine_and_Source")
    os.makedirs(engine_dir, exist_ok=True)
    
    # Subfolder 2: Audio_and_Logs
    audio_dir = os.path.join(dist_dir, "Audio_and_Logs")
    os.makedirs(audio_dir, exist_ok=True)
    
    # 1. Executable outside subfolders at root of Open-Orgel Software folder
    exe_src = os.path.join("dist", "openorgelsynth.exe")
    if not os.path.exists(exe_src):
        exe_src = "openorgelsynth.exe"
    shutil.copy2(exe_src, os.path.join(dist_dir, "openorgelsynth.exe"))
    
    # 2. Subfolder 1 Files (newsynthesis.py, C++ engine, dll, spec)
    for fname in ["newsynthesis.py", "openorgelsynth.cpp", "openorgelsynth.dll", "newsynthesis.spec"]:
        if os.path.exists(fname):
            shutil.copy2(fname, os.path.join(engine_dir, fname))
            
    # 3. Subfolder 2 Files (Audio samples, logs, arduino code, release notes)
    for fname in ["stoppedflue.mp3", "clarion.mp3", "organconsole.ino", "commits_and_releases.txt"]:
        if os.path.exists(fname):
            shutil.copy2(fname, os.path.join(audio_dir, fname))
            
    # Include organ_audio_cache in Audio_and_Logs if present
    if os.path.exists("organ_audio_cache"):
        shutil.copytree("organ_audio_cache", os.path.join(audio_dir, "organ_audio_cache"), dirs_exist_ok=True)
        
    print(f"Created distribution folder: {dist_dir}")
    
    # Create Open-Orgel Software.zip in project folder
    zip_fname = "Open-Orgel Software.zip"
    if os.path.exists(zip_fname):
        os.remove(zip_fname)
        
    with zipfile.ZipFile(zip_fname, 'w', zipfile.ZIP_DEFLATED) as zf:
        for root, dirs, files in os.walk(dist_dir):
            for file in files:
                abs_path = os.path.join(root, file)
                rel_path = os.path.relpath(abs_path, os.path.dirname(dist_dir))
                zf.write(abs_path, rel_path)
                
    print(f"Created distribution zip archive: {zip_fname}")
    
    # Duplicate to Desktop
    desktop_dist_dir = os.path.join(desktop_dir, "Open-Orgel Software")
    desktop_zip_fname = os.path.join(desktop_dir, "Open-Orgel Software.zip")
    
    if os.path.exists(desktop_dist_dir):
        shutil.rmtree(desktop_dist_dir)
        
    shutil.copytree(dist_dir, desktop_dist_dir)
    shutil.copy2(zip_fname, desktop_zip_fname)
    
    print(f"Successfully duplicated distribution package & zip to Desktop:\n - {desktop_dist_dir}\n - {desktop_zip_fname}")

if __name__ == "__main__":
    build_package()
