#!/usr/bin/env python3
"""
OpenOrgel Distribution Packager
Packages the standalone release binary, installer scripts, and assets into a distributable zip archive.
"""

import os
import shutil
import zipfile
import subprocess

def get_git_tag():
    try:
        tag = subprocess.check_output(["git", "describe", "--tags", "--exact-match"], text=True).strip()
        return tag
    except Exception:
        try:
            return subprocess.check_output(["git", "describe", "--tags", "--abbrev=0"], text=True).strip()
        except Exception:
            return "v2.0.5"

def build_package():
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    dist_dir = os.path.join(repo_root, "dist")
    os.makedirs(dist_dir, exist_ok=True)
    
    tag = get_git_tag()
    pkg_name = f"OpenOrgel-{tag}-Windows"
    staging_dir = os.path.join(dist_dir, pkg_name)
    
    if os.path.exists(staging_dir):
        shutil.rmtree(staging_dir)
    os.makedirs(staging_dir, exist_ok=True)
    
    # 1. Locate release binary
    candidate_bins = [
        os.path.join(repo_root, "rust", "target", "x86_64-pc-windows-gnullvm", "release", "openorgelsynth.exe"),
        os.path.join(repo_root, "rust", "target", "release", "openorgelsynth.exe"),
        os.path.join(r"C:\Users", os.getenv("USERNAME", "EME0012"), "OneDrive - St Helena Secondary College", "Desktop", "openorgelsynth.exe")
    ]
    
    exe_path = None
    for c in candidate_bins:
        if os.path.exists(c):
            exe_path = c
            break
            
    if not exe_path:
        raise FileNotFoundError("Could not find release binary openorgelsynth.exe. Run 'cargo build --release' first.")
        
    print(f"Packaging binary from: {exe_path}")
    shutil.copy2(exe_path, os.path.join(staging_dir, "openorgelsynth.exe"))
    
    # 2. Copy installer scripts
    installer_files = ["install.ps1", "uninstall.ps1", "install.cmd", "openorgel_setup.iss"]
    for f in installer_files:
        src = os.path.join(repo_root, "installer", f)
        if os.path.exists(src):
            shutil.copy2(src, os.path.join(staging_dir, f))
            
    # 3. Copy audio samples and docs
    root_files = ["clarion.wav", "stoppedflue.wav", "README.md", "LICENSE", "unit_tests_report.md"]
    for f in root_files:
        src = os.path.join(repo_root, f)
        if os.path.exists(src):
            shutil.copy2(src, os.path.join(staging_dir, f))
            
    # 4. Create zip archive
    zip_path = os.path.join(dist_dir, f"{pkg_name}.zip")
    if os.path.exists(zip_path):
        os.remove(zip_path)
        
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
        for root, _, files in os.walk(staging_dir):
            for file in files:
                abs_path = os.path.join(root, file)
                rel_path = os.path.relpath(abs_path, staging_dir)
                zf.write(abs_path, os.path.join(pkg_name, rel_path))
                
    print(f"Created standalone distribution package: {zip_path} ({os.path.getsize(zip_path)} bytes)")
    return zip_path

if __name__ == "__main__":
    build_package()
